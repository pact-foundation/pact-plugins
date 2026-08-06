# Field-level matchers and generators

> [!NOTE]
> **Implementation phase:** Phase 3 (new functionality). Requires [005](./005_Plugin_capability_negotiation_and_versioning.md) to be finalised. Designed in parallel with [007](./007_Driver_plugin_callback_model.md), and re-uses 007's registry, resolver and callback plumbing rather than introducing its own. Required by [009](./009_Host_provided_core_matching_and_generation.md). See the [proposals README](./README.md) for the full delivery order.

## Summary

Add a plugin API for matching and generating data at the field or element level, instead of limiting plugins to whole
content types and transport-level interactions.

## Problem statement

The catalogue model already recognises matcher entries, but the current runtime interface is focused on content
matchers/generators and transport plugins. This leaves a gap for use cases where a plugin should contribute matching or
generation logic for a specific field, key, header, token, or nested value.

Without an explicit field-level API, plugin authors either cannot express these use cases at all or are pushed into
whole-content plugins that are broader and more complex than the problem requires.

Concretely, today `CatalogueEntry.EntryType.MATCHER` can be registered by a plugin and stored in the catalogue, and
that is all that happens - nothing in either driver ever looks a `MATCHER` entry up or calls the plugin that owns it.
There is also no entry type at all for a field-level generator.

## Worked example: a credit card number

The design below is derived from a reference plugin, [`plugins/creditcard`](../../plugins/creditcard), written in Lua
(the second Lua plugin in this repository, after [`plugins/jwt`](../../plugins/jwt)). It provides one matching rule
and one generator, both named `creditcard`:

- **the matcher** asserts that the actual value is a plausible credit card number: digits only (with `-`/space
  separators tolerated), a valid [Luhn](https://en.wikipedia.org/wiki/Luhn_algorithm) check digit, a length in range,
  and - if the rule is configured with a `brand` - the IIN prefix and length of that brand;
- **the generator** produces a fresh, Luhn-valid number of the same brand on every test run.

This is a deliberately small example that is nonetheless not expressible with any core matching rule: `regex` can
check the shape of a card number but not its check digit, and `type` only gets you "it's a string". It exercises
every part of the interface - rule configuration values, an example value, a value-level mismatch, and a generator
paired with a matcher under the same name - without needing binary values or document context, which keeps it honest
about which parts of the design are load-bearing.

A consumer test asserting on a JSON body would express it as:

```json
{
  "card": {
    "number": "matching(creditcard, 'visa', '4111111111111111')",
    "expiry": "matching(regex, '\\d{2}/\\d{2}', '04/28')"
  }
}
```

which is persisted into the Pact file as an ordinary matching rule whose name happens to be provided by a plugin:

```json
"matchingRules": {
  "body": {
    "$.card.number": {
      "combine": "AND",
      "matchers": [ { "match": "creditcard", "brand": "visa" } ]
    }
  }
}
```

## Design

### 1. Catalogue entries

A field-level matcher is a `MATCHER` catalogue entry (already defined, never previously used):
`plugin/creditcard/matcher/creditcard`.

A field-level generator needs a new entry type, `GENERATOR = 5`, added to `CatalogueEntry.EntryType` in
`proto/plugin_v2.proto` and to `CatalogueEntryType` in both drivers:
`plugin/creditcard/generator/creditcard`.

Reusing `MATCHER` for both operations was considered and rejected: a separate type keeps
`catalogue_manager::resolve_capability`'s `expected_type` check meaningful (it is what stops a callback for a
generator resolving to an unrelated matcher of the same name), and lets a matcher and its paired generator share one
name, which is what a plugin author actually wants.

Unlike `CONTENT_MATCHER`/`CONTENT_GENERATOR`, these entries have no required `values` key - there is no content type
to advertise. One optional convention is defined:

| Key | Meaning |
|---|---|
| `config-key` | The name of the values key that a single positional config argument in a matching rule definition expression maps to (see [3](#3-declaring-a-plugin-rule-in-a-test)). Defaults to `value`. |

The `creditcard` plugin registers `config-key = "brand"`, which is what makes `matching(creditcard, 'visa', '4111…')`
resolve to `{ "match": "creditcard", "brand": "visa" }`.

### 2. Naming and resolution

The **rule name is the catalogue key**. A rule named `creditcard` resolves through the existing
`catalogue_manager::resolve_capability` (`CatalogueManager.resolveCapability` on the JVM) with an expected type of
`MATCHER`/`GENERATOR` - the same resolver 007 already uses for callbacks, with the same behaviour:

- a bare name (`creditcard`) matches the entry's key; a caller can also name more of the catalogue key
  (`matcher/creditcard`, `plugin/creditcard/matcher/creditcard`) to disambiguate if two plugins ever register the
  same short name. Key components are compared whole, never as substrings;
- failing that, the name is matched against the core catalogue's versioned naming convention - the Pact
  specification version a rule was introduced in, prefixed to its name - so `type` resolves to `core/matcher/v2-type`
  and `date` to `core/matcher/v3-date` without a caller having to know which version introduced what. This applies
  only to `MATCHER` and `GENERATOR` entries: content matchers, content generators and transports are registered
  under plain names (`xml`, `json`, `grpc`), where a leading `v<n>-` would be part of the name. Naming an entry
  directly always wins over the fallback: a plugin registering its own `matcher/date` takes `date`, and a caller
  that specifically wants the core rule asks for `v3-date`;
- a name matching more than one entry of the expected type is a hard error naming the candidates, never a silent
  pick;
- a name matching nothing is a hard error at match/generate time.

Core rule names always win: the host resolves `type`, `regex`, `equality` and the rest of the standard set from its
own matching engine before it ever consults the catalogue, so a plugin cannot shadow a standard rule. (Once
[009](./009_Host_provided_core_matching_and_generation.md) registers the standard set as `CORE` entries, those become
visible in the catalogue too - as `core/matcher/v2-type` etc. - which is what lets a *plugin* call back for them, but
does not change how the host resolves its own rules.)

### 3. Declaring a plugin rule in a test

Two forms, both existing mechanisms extended to accept a name the host does not recognise:

**Matching rule definition expression.** `matching(NAME, [CONFIG,] EXAMPLE)` where `NAME` is not one of the built-in
rule names. `CONFIG` is optional and is stored under the key the catalogue entry's `config-key` value names
(defaulting to `value`). The grammar in [`matching-rule-definition.g4`](../matching-rule-definition.g4) gains one
alternative in the `matchingRule` production for this; nothing else changes.

**JSON form.** The existing `pact:matcher:type` mechanism already carries arbitrary attributes, and needs no change
at all beyond the host accepting an unrecognised type:

```json
{ "pact:matcher:type": "creditcard", "brand": "visa", "value": "4111111111111111" }
```

Generators are declared the same way, by name, via `pact:generator:type`:

```json
{ "pact:matcher:type": "creditcard", "pact:generator:type": "creditcard", "brand": "visa", "value": "4111111111111111" }
```

Whether a matching rule should *implicitly* attach the same-named generator when one exists (the way `datetime`
implies a date-time generator today) is left open - see [Open questions](#open-questions). The explicit form above is
the design; an implicit one would be a convenience layered on top of it.

### 4. Model representation and the Pact file

Both the matching rule and the generator are stored in the Pact file exactly as any other rule is - the name in the
`match`/`type` field, its configuration as sibling keys:

```json
"matchingRules": { "body": { "$.card.number": { "matchers": [ { "match": "creditcard", "brand": "visa" } ] } } },
"generators":    { "body": { "$.card.number": { "type": "creditcard", "brand": "visa" } } }
```

This requires a carrier in the models, in `pact_models` and in Pact-JVM's model module:

```rust
MatchingRule::Plugin { name: String, values: Value }
Generator::Plugin { name: String, values: Value }
```

with these properties:

- `MatchingRule::create` falls through to `Plugin { name, values }` only for a rule name it does not recognise -
  a malformed *known* rule (e.g. `regex` with no `regex` field) still errors exactly as it does today, so the
  fallback cannot mask a real parse error. `Generator::from_map` does the same for an unrecognised `type` (today it
  logs a warning and drops the generator silently, which is worse than either alternative).
- `to_json` round-trips: `{ "match": name, ...values }` / `{ "type": name, ...values }`. There is no marker in the
  JSON saying "this is a plugin rule" - by design, since which plugin (if any) provides a name is a property of the
  running catalogue, not of the file.
- `name()`/`values()` return the plugin name and its values map, which means the existing content-matcher path in
  `content.rs` forwards a plugin field rule to a content matcher plugin unchanged, with no code change. A protobuf
  or CSV plugin receiving `{ type: "creditcard", values: { brand: "visa" } }` in its `rules` map can then delegate
  it back to the `creditcard` plugin through the 007 callback (see [7](#7-callbacks-and-host-provided-rules)).
- `can_cascade()` is `false`: a plugin rule applies to the value at its path, not to that value's children.

A typo'd core rule name becomes a `Plugin` rule that fails resolution at match time with
`No catalogue entry found for key 'reges'`, which is an acceptable trade for not needing a second syntax.

**Pact file plugin metadata.** A field-level rule never goes through `configure_interaction`, which is where the
`plugins` entry in the Pact file metadata is written today. So the host must record the providing plugin (name and
version, from the resolved catalogue entry's manifest) in the Pact metadata when it serialises a `Plugin` rule or
generator. Without this, provider verification has no way to know it needs to load the `creditcard` plugin before it
can interpret the file. Consumer tests must load the plugin explicitly (`usingPlugin("creditcard")` or equivalent)
before the rule can be resolved at all.

### 5. Operation shape

Two new RPCs on `PactPlugin`, and their `PluginHost` counterparts for the callback direction:

```proto
// A single value being matched or generated. Each type Pact's matching rules discriminate has its
// own arm - see "Value model" below for why this is not a google.protobuf.Value.
message FieldValue {
  oneof value {
    google.protobuf.NullValue nullValue = 1;
    bool booleanValue = 2;
    string stringValue = 3;
    int64 integerValue = 4;
    double decimalValue = 5;
    bytes binaryValue = 6;
    // A map or list, for a rule applied to a collection rather than a scalar
    google.protobuf.Value structuredValue = 7;
  }
}

message MatchFieldRequest {
  // Catalogue entry key of the rule being applied, e.g. "creditcard"
  string key = 1;
  // The rule as stored in the Pact file: its name and configured values
  MatchingRule rule = 2;
  // Path to the value being matched, as per the documented Pact matching rule expressions
  string path = 3;
  // Which part of the interaction the value came from: body, header, query, metadata, path, status
  string mismatchType = 4;
  // Expected value from the Pact file
  FieldValue expected = 5;
  // Actual value received
  FieldValue actual = 6;
  // Additional data added to the Pact/Interaction by the plugin
  PluginConfiguration pluginConfiguration = 7;
  // Context data provided by the test framework (carries testRunId for log correlation)
  google.protobuf.Struct testContext = 8;
}

message MatchFieldResponse {
  // Set if the match could not be performed at all. If set, mismatches is ignored and the
  // verification is marked as failed.
  string error = 1;
  // Mismatches found. Empty means the value matched. A mismatch with an empty path is
  // reported against the request's path.
  repeated ContentMismatch mismatches = 2;
}

message GenerateFieldRequest {
  string key = 1;
  // The generator as stored in the Pact file: its name and configured values
  Generator generator = 2;
  string path = 3;
  // The example value from the Pact file that the generated value replaces
  FieldValue exampleValue = 4;
  PluginConfiguration pluginConfiguration = 5;
  google.protobuf.Struct testContext = 6;
  // Consumer or provider side, reusing the enum already defined for content generation
  GenerateContentRequest.TestMode testMode = 7;
}

message GenerateFieldResponse {
  string error = 1;
  FieldValue value = 2;
}

service PactPlugin {
  // ... existing RPCs ...

  // Apply a plugin-provided matching rule to a single value. Required for any plugin that
  // registers a MATCHER catalogue entry.
  rpc MatchField(MatchFieldRequest) returns (MatchFieldResponse);
  // Apply a plugin-provided generator to a single value. Required for any plugin that
  // registers a GENERATOR catalogue entry.
  rpc GenerateField(GenerateFieldRequest) returns (GenerateFieldResponse);
}
```

**Value model.** The first draft of this proposal reused the `MetadataValue` shape - a `oneof` of
`google.protobuf.Value` or `bytes`. That is wrong here, and the difference matters: `google.protobuf.Value` has a
single number type (a double), so `100` and `100.0` are indistinguishable once a value crosses the interface, for
every plugin implementation. The rules that would break are exactly the ones whose job is to check a value's runtime
type - `integer` and `decimal` could never tell their two cases apart, and `type` would wrongly pass an actual
decimal against an expected whole number. Under [009](./009_Host_provided_core_matching_and_generation.md) those are
*host* handlers being called *by* a plugin, so the host cannot recover what the boundary erased.

Hence an explicit arm per type. Metadata can afford the loose model because nothing type-checks a metadata value;
a matching interface cannot.

The one accepted imprecision: numbers nested inside `structuredValue` still follow JSON semantics. A rule applied to
a collection only needs its shape and size - the values inside it are matched by their own field-level calls, at
their own paths, where they arrive under the scalar arms.

Notes on specific choices:

- **`rule`/`generator` carry the whole rule, not just its values**, so one plugin can register several related rules
  and dispatch on `rule.type` internally, and so the request is self-describing in a log.
- **`ContentMismatch` is reused verbatim** for results, per this proposal's original direction - a field mismatch and
  a content mismatch are the same thing at different granularity, and every driver, reporter and UI already
  understands the type. `expected`/`actual` on it are bytes, so a binary value survives being reported.
- **No `allow_unexpected_keys`**: that is a whole-content concept.
- **Capability classification** ([005](./005_Plugin_capability_negotiation_and_versioning.md)): no new capability
  strings. A plugin declares it provides a field rule by registering the catalogue entry, and implementing the
  matching RPC is then part of that entry's contract. In the other direction, host-provided rules appear in
  `hostCapabilities` automatically, since the driver already derives that list from its core catalogue entries as
  `<entry_type>/<key>` - `matcher/v2-type`, `generator/v3-date`, and so on, once 009 registers them.

### 6. Context available to the plugin

A field-level call sees the value, its path, the rule's own configuration, the plugin's stored configuration, and the
test context. It deliberately does **not** see the surrounding document, sibling values, or the rest of the
interaction.

This is the answer to the proposal's original open question, and the reasoning is: shipping the enclosing document
would mean serialising it per field (potentially per element of a large collection), and would need a document model
in the request that is neither binary-safe nor transport-neutral - reintroducing exactly the problem the `oneof`
value model exists to avoid. A rule that genuinely needs to reason about more than one value is a *content* matcher,
which is already supported, and which can now delegate individual fields back down to field rules. The dividing line
is: **field rules see one value; content matchers see the document.**

Generators are pure functions of `(generator config, example value, testContext, testMode)`. They may vary their
output between calls (that is the point of a generator), but they must not depend on hidden host state: anything the
host knows that a generator needs - the current time, provider state values, the mock server URL - arrives in
`testContext`, or is fetched explicitly through a 007 callback. This keeps a generator reproducible from what is in
the request, which matters for both drivers and for any future in-process runtime.

### 7. Callbacks and host-provided rules

Both operations are added to the `PluginHost` service as well, so a plugin can invoke a field rule it does not own -
the host's standard `type`/`regex`/`date` implementations from
[009](./009_Host_provided_core_matching_and_generation.md), or another plugin's rule:

```proto
message HostMatchFieldRequest {
  string entryKey = 1;
  MatchFieldRequest request = 2;
}

message HostGenerateFieldRequest {
  string entryKey = 1;
  GenerateFieldRequest request = 2;
}

service PluginHost {
  // ... Log, CompareContents, GenerateContent ...
  rpc MatchField(HostMatchFieldRequest) returns (MatchFieldResponse);
  rpc GenerateField(HostGenerateFieldRequest) returns (GenerateFieldResponse);
}
```

No new plumbing is required: this is 007's mechanism with two more capability shapes. The same `resolve_capability`
resolver, the same `pact-call-chain-id` cycle detection and `pact-deadline-ms` propagation, the same CORE-or-forward
dispatch.

This is what makes the "plugin owns a content type, delegates most fields to the host" story work: a protobuf plugin
matching a `CreditCard` message can apply `matcher/v2-type` to most fields through the host and only implement what is
actually protobuf-specific, and can hand a field carrying a `{ "match": "creditcard" }` rule straight to the
`creditcard` plugin without knowing it exists.

### 8. Driver-side model

Mirroring `content.rs`'s `ContentMatcher`/`ContentGenerator`, a new `field.rs` module in the Rust driver (and
`FieldMatcher.kt`/`FieldGenerator.kt` on the JVM):

```rust
/// A single value being matched or generated - the driver-side counterpart of the proto FieldValue.
/// serde_json::Value already distinguishes a whole number from a decimal, so the driver-side type
/// stays simple; it is the conversion to the proto form that keeps the distinction on the wire.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
  /// A JSON-like value
  Json(serde_json::Value),
  /// Raw bytes
  Binary(Bytes)
}

/// Where a value sits and what is known about it, shared by matching and generation
pub struct FieldContext {
  pub path: DocPath,
  pub category: String,
  pub plugin_config: Option<PluginInteractionConfig>,
  pub test_context: HashMap<String, Value>
}

pub struct FieldMatcher { pub catalogue_entry: CatalogueEntry }

impl FieldMatcher {
  pub fn is_core(&self) -> bool;

  pub async fn match_field(
    &self,
    rule: &MatchingRule,
    expected: &FieldValue,
    actual: &FieldValue,
    context: &FieldContext
  ) -> Result<(), Vec<ContentMismatch>>;
}

pub struct FieldGenerator { pub catalogue_entry: CatalogueEntry }

impl FieldGenerator {
  pub fn is_core(&self) -> bool;

  pub async fn generate_field(
    &self,
    generator: &Generator,
    example: &FieldValue,
    mode: TestMode,
    context: &FieldContext
  ) -> anyhow::Result<FieldValue>;
}

/// Look up a field matcher/generator by rule name (not by content type, unlike
/// find_content_matcher). Returns a descriptive error rather than None, since "no such rule",
/// "ambiguous rule" and "that name is a generator, not a matcher" all need to reach the user.
pub fn find_field_matcher(name: &str) -> anyhow::Result<FieldMatcher>;
pub fn find_field_generator(name: &str) -> anyhow::Result<FieldGenerator>;
```

Both operations also need a blocking wrapper, because every Rust host that applies a matching rule does so from a
synchronous call path - see [9](#9-host-framework-integration) for why that bridge belongs in the driver.

`is_core()` dispatches exactly as `content.rs` does after 007: to a handler registered in `core_capabilities`, or to
the owning plugin over gRPC on a fresh call chain. The two new traits and their registration functions follow the
established shape:

```rust
#[async_trait]
pub trait CoreFieldMatcher: Send + Sync {
  async fn match_field(&self, request: MatchFieldRequest) -> anyhow::Result<MatchFieldResponse>;
}

#[async_trait]
pub trait CoreFieldGenerator: Send + Sync {
  async fn generate_field(&self, request: GenerateFieldRequest) -> anyhow::Result<GenerateFieldResponse>;
}

pub fn register_core_field_matcher(key: &str, handler: Arc<dyn CoreFieldMatcher>);
pub fn register_core_field_generator(key: &str, handler: Arc<dyn CoreFieldGenerator>);
```

These four functions are the entire remaining requirement of
[009](./009_Host_provided_core_matching_and_generation.md) step 2.

### 9. Host framework integration

The work outside this repository, stated explicitly because it is most of the delivery risk:

1. `pact_models` / Pact-JVM model: the `Plugin` carrier variants from [4](#4-model-representation-and-the-pact-file),
   their JSON round-trip, and the definition-expression parser change.
2. `pact_matching` / Pact-JVM matching engine: a dispatch arm that resolves a `Plugin` rule through
   `pact_plugin_driver::field::find_field_matcher` and calls it, and the equivalent for generators in generator
   application. Both crates already depend on the driver, so no new dependency edge is created.
3. Recording the providing plugin in the Pact file's `plugins` metadata when a `Plugin` rule or generator is
   serialised (see [4](#4-model-representation-and-the-pact-file)).

**Calling an async driver from a synchronous matching path (Rust only).** The JVM driver talks to plugins over
blocking gRPC stubs (`PactPluginBlockingStub`), so Pact-JVM has nothing to solve here. In Rust, rule application is
synchronous - `match_values` -> `Matches::matches_with` - and the V2 matching engine's `execute_request_plan`/
`execute_response_plan`/`execute_message_plan` are synchronous end to end, while the driver's plugin calls are async.

Two approaches do **not** work at that call site:

- `Handle::current().block_on(fut)` panics. Body matching is reached from the async `compare_bodies`, so the thread
  is already inside a runtime context, and tokio's `enter_runtime` guard raises *"Cannot start a runtime from within
  a runtime"*.
- `task::block_in_place(|| Handle::current().block_on(fut))` is tokio's documented way to re-enter a runtime, but
  panics on a `current_thread` runtime. Pact has `current_thread` call sites that reach matching - `pact_consumer`'s
  `check_requests_match` drives `match_request` on one - so this would work under the verifier, the FFI and the HTTP
  mock server and panic elsewhere.

What does work is the bridge `pact_matching` already uses twice for multipart bodies (`binary_utils.rs`:
`match_mime_multipart`, `parse_multipart_body`): run the future on a separate thread with its own runtime, and
receive the result over a channel with a timeout. Two refinements over that existing code:

- It calls `Handle::try_current()` *inside* the spawned thread. Tokio's runtime context is a thread-local that
  `std::thread::spawn` does not inherit, so that call always returns `Err` and the "reuse the host runtime" branch
  never actually runs. Capture the handle before spawning if reuse is the intent.
- Reusing the host runtime is only safe when it is a multi-thread one anyway. A plugin's tonic `Channel` spawns its
  connection task on whichever runtime was current when the plugin was loaded; if that is a `current_thread` runtime
  whose only thread is the one now blocked inside sync matching, the connection cannot progress and the call hangs
  until the timeout.

So the driver should own a dedicated runtime for plugin calls and expose blocking wrappers next to the async
operations, rather than leaving each host to build its own bridge:

```rust
pub fn match_field_blocking(/* same arguments as match_field */) -> Result<(), Vec<ContentMismatch>>;
pub fn generate_field_blocking(/* same arguments as generate_field */) -> anyhow::Result<FieldValue>;
```

The wrapper's timeout should come from the existing deadline (`call_chain::default_deadline_ms`) rather than being a
second, unrelated number.

### 10. Lua transport

A Lua plugin defines two more optional globals, required only if it registers the corresponding catalogue entry.
Table shapes mirror the proto fields, following the existing conventions in the
[Lua plugin reference](../lua-plugin-reference.md) - `snake_case` keys, and a field value that is either a plain Lua
value or a `{ binary = "..." }` wrapper, exactly as metadata values already work:

```lua
-- request: { key, rule = { type, values }, path, mismatch_type, expected, actual,
--            plugin_configuration, test_context }
-- returns: { mismatches = { <ContentMismatch table>, ... } } or { error = "..." }
function match_field(request) end

-- request: { key, generator = { type, values }, path, example_value,
--            plugin_configuration, test_context, test_mode }
-- returns: { value = <any> } or { error = "..." }
function generate_field(request) end
```

and gains two host functions alongside `host_compare_contents`/`host_generate_content`:

```lua
host_match_field(entry_key, request)     -- -> { error = "...", mismatches = { ... } }
host_generate_field(entry_key, request)  -- -> { error = "...", value = ... }
```

Both are async host functions resolving through the same `resolve_capability` path as their content-level
equivalents, with no call-chain ID or cycle detection needed - the same reasoning as in 007's Lua section.

### WASM transport

Out of scope for this design pass. The operation shape is transport-neutral by construction (the `oneof` value model
and reused message types exist precisely so it can cross a linear-memory boundary as serialised protobuf), so the
WASM mapping is the mechanical one 007 describes, to be filled in when [003](./003_Support_WASM_plugins.md) lands
WASM plugin support at all.

## Sequencing

1. ✅ Proto: `GENERATOR` entry type, `FieldValue`, the four request/response messages, the two `PactPlugin` RPCs and
   the two `PluginHost` RPCs, with the checked-in Rust bindings regenerated. The Rust driver's `PluginHost` service
   answers the two new RPCs with `unimplemented` until step 2; the JVM driver needs no change for this, since
   gRPC-Java's generated base class already answers an unimplemented method that way.
2. ✅ Rust driver: `field.rs` (`FieldValue`, `FieldContext`, `FieldMatcher`/`FieldGenerator` and their blocking
   wrappers), the two `core_capabilities` traits + registries, catalogue lookups via
   `catalogue_manager::resolve_capability_entry`, the `PluginHost` service methods, and the
   `grpc_plugin`/`PluginInstance` methods (V2-only - a V1 plugin gets a clear error, since the operations do not
   exist on that interface). `proto_v2` had to become a public module: the embedding framework needs those types to
   implement `CoreFieldMatcher`/`CoreFieldGenerator`.

   One trap here: the driver's internal catalogue representation is the **V1** proto's `EntryType`
   (`CatalogueEntryType::to_proto_type`, and the byte-level transcode of V2 catalogue entries into V1
   `CatalogueEntry` messages in `grpc_plugin.rs`), while `GENERATOR` only exists in V2. The raw `i32` survives the
   transcode, but prost's generated `entry.r#type()` accessor maps an unrecognised `5` to the default
   (`ContentMatcher`) silently, so the inbound path must read the raw field. The clean fix is to stop treating a
   generated proto enum as the driver's own type - `CatalogueEntryType` should be the source of truth - rather than
   adding `GENERATOR` to the frozen V1 contract.
3. ✅ JVM driver: the same surface, in `Field.kt` (`FieldValue`, `FieldContext`, `FieldMatcher`/`FieldGenerator`),
   `CoreCapabilities.kt`, `CatalogueManager.resolveCapabilityEntry`, `PluginHostServer.kt` and
   `PluginRpcClient.kt`. No blocking wrapper is needed here - the JVM driver talks to plugins over blocking gRPC
   stubs, so the synchronous matching path calls straight through.
4. ✅ Lua runtime in both drivers: `match_field`/`generate_field` invocation, `host_match_field`/
   `host_generate_field` host functions, conversions in `lua_plugin.rs` / `LuaConversions.kt`.

   The JVM driver needed a fix one level down, in `LuaJavaEngine`. luajava's `toObject` returns every Lua number
   as a `Double` and every Lua string as a Java `String`, so a whole number came back as a decimal and a binary
   value was truncated at its first NUL byte - the two things `FieldValue`'s per-type arms exist to prevent.
   Reading the stack directly (`isInteger`/`toInteger`, and `toBuffer` for a `{ binary = ... }` wrapper) fixes
   both, for the existing metadata path as well as for field values. The Rust driver needed no equivalent: mlua
   distinguishes `Value::Integer` from `Value::Number`, and its strings are byte-safe.
5. ✅ Reference plugin: [`plugins/creditcard`](../../plugins/creditcard), written against this design. Now runs -
   both drivers' test suites load it and exercise its rule and generator end to end - but cannot be reached from a
   consumer test until step 6.
6. ✅ Host framework integration ([9](#9-host-framework-integration)), and an example consumer/provider pair under
   [`examples/creditcard`](../../examples/creditcard), which both hosts generate a Pact from and the same provider
   verifies.

   In `pact-reference`: the `Plugin` carrier variants and their JSON round-trip in `pact_models`, the
   definition-expression production, dispatch from the `DoMatch` implementations in `pact_matching`, a
   `PluginRule` pattern and `body_generator` in `pact_consumer`, and rule id 24 in `pact_ffi`. In Pact-JVM:
   `PluginMatcher`/`PluginGenerator` in `core:model`, a `domatch` branch and `PluginFieldSupport` in
   `core:matchers`, and `PluginRuleMatcher`/`PactDslJsonBody.pluginValue` in `consumer`.

   Two things had to be solved that section 9 did not anticipate:

   - **`values()` can not describe a plugin rule.** `MatchingRule::values()` and `Generator::values()` return
     `HashMap<&'static str, Value>`, and a plugin rule's configuration keys are only known at runtime. Both enums
     gained `value_map() -> HashMap<String, Value>` and `values()` is deprecated; the driver's `field.rs` and
     `content.rs` read the new one, so a plugin rule's configuration survives being forwarded either to its own
     plugin or to a content matcher.
   - **Neither host passes the path down to where a rule is applied.** `DoMatch::match_value` takes the two values
     and nothing else, and `GenerateValue::generate_value` takes neither the path nor the test mode - but a
     field-level request carries all three. Rather than change traits with a dozen implementations each, the places
     that do know push a scope for the duration of the call: `match_values` and the matching engine's interpreter
     in `pact_matching`, `apply_generators` and the `ContentTypeHandler`s in `pact_models`, and
     `JsonContentTypeHandler.applyKey` on the JVM.

   A consequence worth stating plainly: an unrecognised rule or generator name is no longer dropped or downgraded.
   `MatchingRule::create` used to error, `Generator::from_map` used to warn and drop, and Pact-JVM used to fall back
   to `EqualsMatcher` - all three now produce a `Plugin` carrier, so a typo'd rule name fails at match time with
   `No catalogue entry found for key 'reges'` rather than silently matching everything.
7. ✅ Docs: the [Lua reference](../lua-plugin-reference.md) and [plugin writing guide](../writing-plugin-guide.md)
   gain the two new functions, the `MATCHER`/`GENERATOR` entry types and the field value shape. They also gain
   007's `host_compare_contents`/`host_generate_content`, which had never been documented - leaving those out
   while documenting their field-level counterparts would have made the set look arbitrary.

## Non-goals for this proposal

- Redesigning whole-content matcher/generator flows.
- Defining a general-purpose callback bus between plugins and the host (see [007](./007_Driver_plugin_callback_model.md)).
- Solving plugin runtime/version negotiation on its own (see [005](./005_Plugin_capability_negotiation_and_versioning.md)).
- Giving field-level plugins access to the surrounding document (see [6](#6-context-available-to-the-plugin)).
- Rules whose semantics are collection-wide (`arrayContains`, `eachKey`/`eachValue`, `atLeast`/`atMost`). These stay
  core: they are structural rules the matching engine has to interpret itself in order to know *which* values to
  match, and a plugin cannot participate in that decision through a one-value-at-a-time interface.

## Resolved questions

- **What value model should be used for binary-safe field-level matching and generation?** A `FieldValue` `oneof`
  with an explicit arm per type Pact's matching rules discriminate - null, boolean, string, integer, decimal, bytes,
  and a `google.protobuf.Value` for collections. Not the `MetadataValue` shape this proposal originally specified:
  see [5](#5-operation-shape) for why a single JSON-ish value type breaks the type-checking rules. In Lua, the
  existing metadata convention carries over for the scalar/binary split (a plain value, or `{ binary = "..." }`),
  with Lua 5.4's own integer/float distinction (`math.type`) carrying the numeric one.
- **How much of the surrounding document context should be visible to a field-level plugin call?** None. See
  [6](#6-context-available-to-the-plugin) - the line between a field rule and a content matcher is exactly that a
  field rule sees one value.
- **Should field-level generators be pure functions, or can they depend on host-provided context?** Pure functions of
  the request, where the request includes `testContext` and `testMode`. No hidden host state; anything the host knows
  is passed in or fetched through an explicit 007 callback.
- **How is a plugin rule named and resolved?** The rule name *is* the catalogue key, resolved through 007's existing
  `resolve_capability` with its suffix matching, ambiguity detection and clear errors. Core rule names are resolved
  by the host first and cannot be shadowed.
- **Do field-level generators need their own catalogue entry type?** Yes - `GENERATOR = 5`. Reusing `MATCHER` would
  weaken the expected-type check that stops a callback dispatching to the wrong capability shape.
- **Do the new operations need new capability strings under 005?** No. Registering the catalogue entry is the
  declaration; host-provided rules already surface in `hostCapabilities` via the driver's existing
  `<entry_type>/<key>` derivation from core catalogue entries.
- **How does a synchronous Rust matching path call an async plugin operation?** Through a blocking wrapper the
  driver owns, backed by its own runtime - not `Handle::block_on` (panics: the thread is already inside a runtime)
  and not `block_in_place` (panics on the `current_thread` runtimes some Pact entry points use). See
  [9](#9-host-framework-integration). The JVM driver is unaffected, since it uses blocking gRPC stubs.

## Open questions

- **Should a matching rule implicitly attach the same-named generator when the catalogue has one?** It would match
  how `datetime` behaves today and remove a line of boilerplate, but the definition-expression parser lives in
  `pact_models`, which has no visibility of the catalogue - so the resolution would have to happen in a later host
  layer that does. Explicit declaration is the design; this is a possible convenience on top.
- **Pact file portability.** A Pact file containing a plugin rule cannot be interpreted by an implementation that
  cannot load the plugin. The `plugins` metadata entry makes the requirement explicit and diagnosable, but there is
  no graceful degradation (no "fall back to `type` if the plugin is unavailable"). Is that acceptable, or should a
  plugin rule be able to declare a core fallback rule for readers that cannot resolve it?
