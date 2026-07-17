# Driver-plugin callback model (Draft)

> [!NOTE]
> **Implementation phase:** Phase 3 (new functionality). Requires [005](./005_Plugin_capability_negotiation_and_versioning.md) to be finalised. Design in parallel with [006](./006_Field_level_matchers_and_generators.md). Required by [009](./009_Host_provided_core_matching_and_generation.md). See the [proposals README](./README.md) for the full delivery order.

## Summary

Define how a plugin can call back into the driver or host framework for shared functionality, including core Pact
features and, potentially, functionality exposed by other plugins.

## Problem statement

The original plugin design described the driver as a central hub, but the current gRPC interface is still largely a
unary request/response model. That means plugins either need to duplicate functionality already present in the host
framework, or they cannot reuse functionality across plugins at all.

This is especially relevant for richer matcher/generator use cases and for any future in-process plugin runtime.

### The dependency-cycle constraint

The driver crate deliberately depends only on `pact_models` (data types), not on `pact_matching` or any other crate
that implements actual Pact matching/generation behaviour. Those higher-level crates depend on the driver (so they can
call out to plugins), so the reverse dependency is not available: the driver cannot link against the code that knows
how to compare an XML body, apply a `DateGenerator`, or run the standard matching rule set. The same constraint holds
for the JVM driver, which depends on `au.com.dius.pact.core:model` but not on the Pact-JVM matching engine.

This means the driver can advertise that a capability exists (it already does this via the catalogue and
`hostCapabilities` in `InitPlugin`, see [005](./005_Plugin_capability_negotiation_and_versioning.md)), but it cannot
itself execute that capability. Something registered at runtime by the embedding framework has to do the work, and the
driver has to be able to invoke it without knowing its concrete type.

## Recommended direction

### Logical model

Callbacks are modelled as specific typed host capabilities, not a generic message envelope. Each capability is a
well-defined operation with a typed request and response. The driver advertises which capabilities it supports during
the `InitPlugin` handshake (via [005](./005_Plugin_capability_negotiation_and_versioning.md)), so a plugin knows before making any call whether a given capability is available.

### Breaking the dependency cycle: registered handlers, not linked implementations

This proposal does not introduce a new pattern — it generalises one already shipped in
[008](./008_Plugin_observability_and_logging.md). The `PluginLogSink` trait in
`drivers/rust/driver/src/plugin_log_sink.rs` is the working example: the driver defines the trait, holds a global
replaceable instance, and exposes a `set_plugin_log_sink()` registration function. The embedding framework implements
the trait and registers itself once at startup. The driver's compiled code never references the concrete
implementation. The JVM driver's `PluginHostServer` object (instance registry keyed by plugin instance ID) is the same
shape for a simpler case.

The callback model reuses this exactly, generalised from "one sink" to "one handler per capability":

- The driver defines one narrow trait per **capability shape** — not one generic `dyn Any` handler. A capability shape
  corresponds to an operation already defined for plugins (e.g. `CompareContents`, `GenerateContent`) or one that
  [006](./006_Field_level_matchers_and_generators.md) adds for field-level operations. Reusing the existing typed
  messages (`CompareContentsRequest`/`Response`, `GenerateContentRequest`/`Response`, `ContentMismatch`, etc.) means the
  driver→host interface and the driver→plugin interface share a data model — a capability looks the same regardless of
  who provides it.

  ```rust
  #[async_trait]
  pub trait CoreContentMatcher: Send + Sync {
    async fn compare_contents(&self, request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse>;
  }

  #[async_trait]
  pub trait CoreContentGenerator: Send + Sync {
    async fn generate_content(&self, request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse>;
  }
  ```

- A per-trait registry, keyed by the unprefixed catalogue entry key (mirroring `register_core_entries`'s own keying),
  lives next to `CATALOGUE_REGISTER` in `catalogue_manager.rs`:

  ```rust
  pub fn register_core_content_matcher(key: &str, handler: Arc<dyn CoreContentMatcher>);
  pub fn register_core_content_generator(key: &str, handler: Arc<dyn CoreContentGenerator>);
  ```

  Registration happens at the same call site as `register_core_entries`, so a `CatalogueEntryProviderType::CORE` entry
  and its handler are always registered together and can't drift apart.

- The JVM driver mirrors this with `CoreContentMatcher`/`CoreContentGenerator` interfaces and a `CoreCapabilityRegistry`
  object shaped like `PluginHostServer`.

### One resolver, two call directions

Everything above solves how the host registers a capability. What was still missing is how *anything* — the driver
itself, or a plugin calling back — invokes it. Both cases must resolve a catalogue key to either "call the registered
core handler" or "forward to the plugin that owns this entry", and there should be exactly one place that makes that
decision.

**Direction A — the driver's own outbound calls.** `content.rs`'s `ContentMatcher`/`ContentGenerator` already have an
`is_core()` check, added in anticipation of this work, but `match_contents`/`generate_content` currently
`.expect("Plugin type is required")` and panic if `is_core()` is true — there was nothing to call yet. This proposal
closes that gap directly:

```rust
if self.is_core() {
  let handler = core_capabilities::lookup_core_content_matcher(&self.catalogue_entry.key)
    .ok_or_else(|| anyhow!("No core handler registered for '{}'", self.catalogue_entry.key))?;
  handler.compare_contents(request).await
} else {
  // existing lookup_plugin(...) gRPC path, unchanged
}
```

No gRPC is involved here: the driver and the registered handler are in the same process.

**Direction B — a plugin calling back.** This is the actual subject of this proposal. A plugin (external gRPC process
or, per [003](./003_Support_WASM_plugins.md), an in-process WASM module) needs a capability it doesn't implement
itself — for example a field-level plugin from 006 wants the host's standard `type` matcher for one field of a larger
document it otherwise owns. It calls back with a catalogue entry key. The driver resolves that key exactly the way
Direction A does:

- **`CORE`** → call the registered handler in-process, same as Direction A.
- **`PLUGIN`**, owned by a *different* plugin than the caller → forward the call over gRPC to that plugin, using the
  existing `lookup_plugin` mechanism already used for driver→plugin calls. This makes cross-plugin capability calls
  possible: plugin A can transparently use a capability plugin B registered, mediated entirely by the driver. Neither
  plugin needs to know about the other directly.
- **Not found** → fail the callback with a clear error; the plugin surfaces this as a failure in the parent request.

### gRPC transport

The driver implements the callback RPCs as an extension of the `PluginHost` gRPC service introduced in
[008](./008_Plugin_observability_and_logging.md) (currently just `Log`) on the same local listener, whose address is
already passed to the plugin via the `PACT_PLUGIN_HOST` environment variable. Each callback is a normal blocking unary
RPC from plugin to driver — no bi-directional streaming.

```proto
service PluginHost {
  rpc Log(LogMessage) returns (google.protobuf.Empty);

  // New in this proposal:
  rpc CompareContents(HostCompareContentsRequest) returns (CompareContentsResponse);
  rpc GenerateContent(HostGenerateContentRequest) returns (GenerateContentResponse);
  // Further RPCs land here as 006 defines field-level operation shapes.
}

message HostCompareContentsRequest {
  // Catalogue entry key being invoked, e.g. "xml" for content-matcher/xml. Resolved with the
  // same lookup used for plugin-provided entries today.
  string entryKey = 1;
  CompareContentsRequest request = 2;
}

message HostGenerateContentRequest {
  string entryKey = 1;
  GenerateContentRequest request = 2;
}
```

A concrete flow during `VerifyInteraction`, where a field-level plugin delegates one field's matching to a host-provided
matcher:

```
Driver ──── VerifyInteraction(request) ────────────────→ Plugin
                                                         processes...
Plugin ──── CompareContents(entryKey="xml") ───────────→ PluginHost (driver)
                                                         driver resolves entryKey:
                                                           CORE  -> call registered CoreContentMatcher in-process
                                                           PLUGIN -> forward to the owning plugin over gRPC
Plugin ←─── CompareContentsResponse ───────────────────── driver
                                                         continues...
Driver ←─── VerifyInteractionResponse ─────────────────── Plugin
```

Each callback completes before the plugin continues. The call stack is synchronous and nested, which makes error
propagation and deadline tracking straightforward.

**Baseline vs. optional:** the `PluginHost` service itself, and the ability to resolve a catalogue key via it, is a
**baseline V2 capability** per [005](./005_Plugin_capability_negotiation_and_versioning.md)'s classification rule —
its absence would make the protocol structurally incomplete, not just degrade one feature. Any V2 driver must expose
it. Individual entries behind it (e.g. whether `content-matcher/xml` specifically is registered) remain **optional** —
a plugin that needs a specific capability checks for it in `hostCapabilities` at `InitPlugin` time, same as today.

### Cycle detection and deadlines (gRPC only)

**Cycle detection.** A call-chain ID is generated by the driver at the root of any call that may trigger callbacks
(`CompareContents`, `ConfigureInteraction`, `GenerateContent`, `VerifyInteraction`, `PrepareInteractionForVerification`)
and sent as gRPC request metadata (`pact-call-chain-id`). A plugin forwards the same chain ID as metadata on any
callback it makes. The driver keeps an in-memory stack per chain ID (`chain_id -> Vec<entry_key>`) in a new
`call_chain` module:

- Before dispatching a call for `entry_key` under `chain_id`, push `entry_key` onto that chain's stack; if it's already
  present, reject immediately with a cycle error instead of forwarding.
- Pop it when the call completes (success or failure).

This applies identically to Direction B forwarding to another plugin: the driver pushes the target entry key before
forwarding, so a cycle across two or more plugins is caught the same way a self-cycle is.

**Deadlines.** The driver sets an absolute deadline (`pact-deadline-ms`, Unix epoch milliseconds) as metadata on the
root call. Every hop — the plugin's outbound callback, and any forwarding the driver does on the plugin's behalf —
reads it, fails fast if it has already passed, and uses the remaining budget (`deadline_ms - now`) as the timeout for
its own call (`tonic`'s `.timeout()` on the client). A callback can never outlive the request that triggered it.

**Unavailable target:** if the driver's `PluginHost` service is unreachable, the plugin must fail the parent request
with a clear error rather than hanging.

### WASM transport

Host capabilities are exposed as host-exported functions that the WASM module imports at load time. The exported
function signatures correspond directly to the capability traits above (the same `CoreContentMatcher`/
`CoreContentGenerator` etc. are called directly — no gRPC serialisation, though the request/response types can still be
passed as serialised protobuf bytes across the WASM linear-memory boundary, since WASM has no native way to pass Rust
structs by reference). Calls resolve via the native call stack — there is no network hop and no blocking concern. A
true cycle would manifest as a stack overflow, which the WASM runtime handles. No explicit cycle detection or
call-chain metadata is needed for WASM.

### Lua transport (in-process, shipped ahead of WASM)

The Lua plugin runtime (`drivers/rust/driver/src/lua_plugin.rs`) already runs in-process and already registers host
functions into the script's global table (`logger`, `rsa_sign`, etc., see `register_host_functions`). The callback
model extends this the same way as WASM: a `host_compare_contents(entry_key, table)` Lua global that runs the same
resolver as Direction A/B and converts through the conversion helpers `lua_plugin.rs` already has
(`compare_request_to_lua`, `lua_to_compare_response`). Like WASM, no chain ID or cycle detection is needed — it's a
direct, synchronous Rust function call from the Lua VM's perspective.

The logical capability interface — what can be called, what parameters it takes, what it returns — is identical
between gRPC, WASM, and Lua. Only the transport differs.

### Sequencing

This proposal ships as one vertical slice through the mechanism, not the full capability surface:

1. The registry/trait pattern (`core_capabilities` module, generalising `PluginLogSink`).
2. The extended `PluginHost` gRPC service with cycle detection and deadline propagation.
3. Wiring the two existing `is_core()` branches in `content.rs` to call through instead of panicking.
4. The Lua and WASM host-function equivalents.

This is deliberately the smallest slice that proves the mechanism end-to-end, because `is_core()` already exists as an
unfinished seam. [006](./006_Field_level_matchers_and_generators.md) then adds new capability trait shapes
(field-level matching/generation) on top of the same registry and the same `PluginHost` extension pattern — no new
plumbing is needed for it. [009](./009_Host_provided_core_matching_and_generation.md) is, in turn, just "register the
standard Pact matcher/generator set as `CoreContentMatcher`/field-level handlers using this mechanism" — see that
proposal for details.

## Non-goals for this proposal

- Defining the detailed payload model for verification.
- Solving observability/logging by itself (see [008](./008_Plugin_observability_and_logging.md), already implemented).
- Redesigning plugin discovery or packaging.
- Defining the field-level operation shapes themselves (see [006](./006_Field_level_matchers_and_generators.md)) —
  this proposal defines the mechanism they will be registered and invoked through.

## Resolved questions

- **Which specific host capabilities should be exposed first?** Content-level `CompareContents`/`GenerateContent`,
  because the `is_core()` seam for these already exists and is the smallest slice that exercises the full mechanism
  (registry, cycle detection, deadlines, all three transports). Field-level capabilities follow once
  [006](./006_Field_level_matchers_and_generators.md) defines their shape.
- **Generic envelope vs. typed capabilities?** Typed, one trait/RPC pair per capability shape, reusing existing
  message types (`CompareContentsRequest`, `ContentMismatch`, etc.) rather than introducing a parallel generic
  request/response model.
- **How is the dependency cycle avoided?** By generalising the `PluginLogSink` registration pattern from
  [008](./008_Plugin_observability_and_logging.md): the driver defines traits and a registry, the embedding framework
  implements and registers handlers at startup, and the driver never has a compile-time dependency on the
  implementation.
- **Are cross-plugin calls (plugin A using a capability plugin B provides) in scope?** Yes. The resolver that answers
  "who provides this catalogue entry" doesn't distinguish CORE-forwarding from PLUGIN-forwarding as separate
  mechanisms — supporting one means supporting both, and cycle detection is required as soon as any callback exists
  regardless of who's on the other end.
