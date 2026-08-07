# Host-provided core matching and generation (In progress)

> [!NOTE]
> **Implementation phase:** Phase 4. Its prerequisites - [005](./005_Plugin_capability_negotiation_and_versioning.md), [006](./006_Field_level_matchers_and_generators.md) and [007](./007_Driver_plugin_callback_model.md) - are all implemented. Both hosts now register the standard matching rules and generators, at content and field level, so a plugin can delegate a standard rule to the host it is running under. What remains is the WASM transport, which is blocked on [003](./003_Support_WASM_plugins.md). See [Sequencing](#sequencing) for the current state and the [proposals README](./README.md) for the full delivery order.

## Summary

Allow plugins to delegate standard Pact matching and generation behaviour back to the host framework, instead of having
to embed or re-implement that logic themselves.

## Problem statement

Plugin authors currently need to reproduce Pact matching and generation logic that already exists in the host runtime.
In practice, this can force a plugin to depend on a host implementation such as Pact-JVM just to access standard Pact
matchers, generators, and related data model behaviour.

That creates unnecessary duplication, increases plugin complexity, and weakens the portability story for plugins. It
also means plugins can drift from the behaviour of the host they are running under, even when the host already has the
correct implementation of the standard Pact rules.

This becomes particularly important for plugins that work at the field or element level, where the plugin may only need
specialised handling for a subset of the data and should be able to delegate the rest to the host.

## Relationship to 006 and 007

This proposal states the problem from a plugin author's point of view. It does not need a separate mechanism of its
own — it is addressed entirely by combining:

- [007 Driver-plugin callback model](./007_Driver_plugin_callback_model.md), which defines *how* a plugin calls back
  into host-provided functionality without the driver taking on a compile-time dependency on the implementation (the
  `CoreContentMatcher`/`CoreContentGenerator` trait + registry pattern, and the resolver that a plugin's callback goes
  through via the `PluginHost` gRPC service, WASM host functions, or Lua host functions).
- [006 Field-level matchers and generators](./006_Field_level_matchers_and_generators.md), which defines *what shape*
  the operation takes when the granularity is a single field, key, header, or nested value rather than a whole content
  type.

Concretely, "host-provided core matching and generation" means: the host framework registers the standard Pact matcher
and generator set (`type`, `regex`, `equality`, `date`/`time`, etc. — the same set already represented in the core
catalogue) as `CatalogueEntryProviderType::CORE` entries, each with a registered handler implementing the relevant
trait from 007 (a content-level `CoreContentMatcher`/`CoreContentGenerator`, or the field-level equivalent 006 defines).
A plugin that wants standard behaviour for a field it doesn't want to reimplement calls back through the mechanism 007
already defines, naming the catalogue key for the standard rule it wants (`type`, `regex`, `content-type` - the name
the rule carries in a request, or more of the catalogue key to disambiguate it, `matcher/type`/`core/matcher/type` -
see [006](./006_Field_level_matchers_and_generators.md#2-naming-and-resolution)).

No new protocol messages, transports, or dependency-inversion mechanism are needed beyond what 006 and 007 already
define. This proposal's remaining job is narrower than originally scoped: registering the existing standard matcher/
generator set through the 007 mechanism, and confirming the 006 field-level shape carries what standard rules need
(match/generate against a single value, given its path, the matching rule's configured values, and surrounding
context where the rule requires it — e.g. a `date` generator needing the current time, or a rule needing sibling
values).

### Sequencing

1. ✅ 007's content-level mechanism (the `CoreContentMatcher`/`CoreContentGenerator` registry and the extended
   `PluginHost` gRPC service with cycle detection and deadline propagation). Done in both drivers — Rust
   (`core_capabilities.rs`, `call_chain.rs`, `plugin_host.rs`) and JVM (`CoreCapabilities.kt`, `CallChain.kt`,
   `PluginHostServer.kt`) — see [007](./007_Driver_plugin_callback_model.md#sequencing).
2. ✅ [006](./006_Field_level_matchers_and_generators.md)'s field-level operation shape, in both drivers and both
   hosts — see [006's sequencing](./006_Field_level_matchers_and_generators.md#sequencing). What this proposal
   needs from it is the four registration functions and the resolver behind them:
   `register_core_field_matcher`/`register_core_field_generator` (`core_capabilities.rs`) and
   `CoreCapabilityRegistry.registerFieldMatcher`/`registerFieldGenerator` (`CoreCapabilities.kt`), dispatched to by
   `FieldMatcher`/`FieldGenerator` when the resolved entry is a `CORE` one.
3. ✅ Register the standard Pact matcher/generator set as `CORE` catalogue entries with handlers implementing 007's
   traits:
   - **Catalogue entries.** Both hosts register one `MATCHER` entry per matching rule and one `GENERATOR`
     entry per generator they implement — see [Registered entries](#registered-entries) below.
   - **Content-level handlers.** `pact_matching`'s `core_capabilities.rs` registers `xml`, `json`, `text` and
     `multipart-form-data` matchers plus `json` and `binary` generators; Pact-JVM's
     `MatchingConfig.registerCoreCapabilities` registers the same four matchers plus `form-urlencoded`, and the
     `json` generator.
   - **Field-level handlers.** One handler serves every delegatable rule (the rule to apply comes from the
     request, so there is nothing per-rule to write) and one serves every delegatable generator, registered
     against each key: `CoreFieldRuleMatcher`/`CoreFieldValueGenerator` in `pact_matching`'s
     `core_capabilities.rs` and in Pact-JVM's `CoreFieldCapabilities.kt`. The collection-wide rules get
     `CollectionRuleMatcher`/`CollectionValueGenerator`, which answer with why the rule can not be applied to one
     value — see [Delegatable set](#delegatable-set).
4. ⬜ WASM host-function equivalents, following [007](./007_Driver_plugin_callback_model.md#sequencing)'s step 4.
   Not started, and blocked on [003](./003_Support_WASM_plugins.md) landing WASM plugin support at all. The Lua
   equivalents (`host_match_field`/`host_generate_field`) are done in both drivers, as 006 step 4.

### Registered entries

Both hosts key each entry by **the name the rule or generator carries in a request** — the string
`MatchingRule::name()`/`MatchingRule.name` and `Generator::name()`/`Generator.type` return, which is exactly what the
driver puts in `MatchFieldRequest.rule.type` and `GenerateFieldRequest.generator.type`. A plugin handed a rule it
does not implement can therefore name it straight back with no translation. The Pact specification version the rule
was introduced in is a `spec-version` value on the entry (`V1`…`V4`) rather than a prefix on the key.

| | `pact_matching` (`matchingrules.rs`) | Pact-JVM (`MatcherExecutor.kt`) |
|---|---|---|
| `MATCHER` | `equality`, `regex`, `type`, `min-type`, `max-type`, `min-max-type`, `include`, `number`, `integer`, `decimal`, `null`, `date`, `time`, `datetime`, `content-type`, `values`, `array-contains`, `boolean`, `status-code`, `not-empty`, `semver`, `each-key`, `each-value` | the same 23, plus `ignore-order`, `min-ignore-order`, `max-ignore-order`, `min-max-ignore-order` |
| `GENERATOR` | `RandomInt`, `RandomDecimal`, `RandomHexadecimal`, `RandomString`, `RandomBoolean`, `Regex`, `Uuid`, `Date`, `Time`, `DateTime`, `ProviderState`, `MockServerURL`, `ArrayContains` | the same 13, plus `Null` |

Each list is exactly what that host implements — the differences are rules and generators the other genuinely does
not have, not drift, and a test in each host pins the entry keys to the implemented set so a new rule cannot be added
without an entry. Generator keys are `PascalCase` where matching rule keys are kebab-case, because that is what each
carries on the wire; matching what a plugin was handed matters more than looking uniform.

### Delegatable set

Registering an entry says the capability exists; registering a handler says it can be called with one value. Those
are not the same set, and step 3 should not pretend otherwise:

- **Delegatable** — the rules whose semantics are "look at this one value": `equality`, `regex`, `type`, `include`,
  `number`, `integer`, `decimal`, `boolean`, `null`, `date`, `time`, `datetime`, `content-type`, `not-empty`,
  `semver`, `status-code`. These get a `CoreFieldMatcher`. Likewise the generators that are a pure function of their
  configuration and the test context: `RandomInt`, `RandomDecimal`, `RandomHexadecimal`, `RandomString`,
  `RandomBoolean`, `Regex`, `Uuid`, `Date`, `Time`, `DateTime`, `Null`, and `ProviderState`/`MockServerURL` provided
  the value they need is in `testContext` (see [006 §6](./006_Field_level_matchers_and_generators.md#6-context-available-to-the-plugin)).
- **Not delegatable** — the collection-wide rules [006 lists as non-goals](./006_Field_level_matchers_and_generators.md#non-goals-for-this-proposal):
  `min-type`, `max-type`, `min-max-type`, `values`, `array-contains`, `each-key`, `each-value`, and Pact-JVM's four
  `*ignore-order` rules; and the `ArrayContains` generator, which generates *into* a structure the caller owns.
  A plugin cannot participate in "which values does this apply to" through a one-value-at-a-time interface. These
  keep their catalogue entry — they are real capabilities the host has, and 005 discovery should see them — but a
  callback naming one should fail with a message saying the rule is collection-wide and cannot be applied to a
  single value, not with the generic "no handler registered".

Every entry a host advertises in `hostCapabilities` (both drivers derive it from the core catalogue as
`<entry_type>/<key>`) now has a handler behind it, so a plugin that takes `matcher/type` or `generator/Uuid` at its
word gets an answer rather than *"No core field matcher registered"*. A test in each host compares the registered
handler keys against the catalogue entry keys, so the two can not drift apart.

Two things the handlers have to get right, both a consequence of what the interface can carry:

- **A rule this framework does not provide is an error, not a call back out.** An unrecognised rule name parses into
  the `Plugin` carrier variant 006 added, which would send the call straight back to a plugin - so the handlers
  check the name against the core catalogue first and reject anything that is not theirs.
- **Configuration numbers arrive as doubles.** A rule's or generator's configuration crosses the interface as a
  `google.protobuf.Struct`, which has a single number type, so a `min` of `2` arrives as `2.0` - and both hosts read
  those attributes with integer accessors that reject a float and fall back to the attribute's default. A
  `RandomInt(5, 5)` from a plugin would have generated from `0..10`. Both handlers put whole floats back to integers
  on the way in. This is the configuration-value counterpart of the per-type `FieldValue` arms that keep the *value*
  being matched from going through the same lossy step.

## Recommended direction

- Treat standard Pact matching and generation as host capabilities registered through the [007](./007_Driver_plugin_callback_model.md)
  mechanism — no bespoke registration path.
- Build on [006](./006_Field_level_matchers_and_generators.md)'s field-level operation shape so plugins can delegate at
  the granularity they actually need (a single field), not just whole content types.
- Start with the existing standard Pact matcher and generator set already represented in the core catalogue.
- Host behaviour remains the source of truth for standard Pact semantics: the plugin calls the host's registered
  handler rather than the host validating or overriding a plugin's own implementation of a standard rule.
- The interface is transport-agnostic by construction, since it's the same 007 mechanism used for:
  - external gRPC plugins (via the extended `PluginHost` service);
  - in-process WASM plugins (see [003](./003_Support_WASM_plugins.md), via host-exported functions);
  - in-process Lua plugins (via host functions registered into the Lua VM).

## Non-goals for this proposal

- Defining the generic callback protocol on its own (see [007](./007_Driver_plugin_callback_model.md)).
- Defining new field-level matcher/generator operations from scratch (see [006](./006_Field_level_matchers_and_generators.md)).
- Replacing all plugin logic with host-side logic; plugins should still own their specialised behaviour.

## WASM compatibility

WASM plugins are the primary beneficiary of this proposal. A WASM module cannot link to native Pact libraries, so host-provided matching and generation is essential for non-trivial WASM plugins rather than a convenience. The problem described in this proposal — plugins reproducing logic that already exists in the host — is most acute for WASM.

This maps directly onto [007](./007_Driver_plugin_callback_model.md)'s WASM transport section: the standard matcher/
generator handlers are called as host-exported functions, with no network hop and no cycle-detection bookkeeping
required.

## Relationship to other proposals

- [006 Field-level matchers and generators](./006_Field_level_matchers_and_generators.md) defines where plugin-owned
  matching and generation hooks are needed, and the operation shape standard rules are delegated through.
- [007 Driver-plugin callback model](./007_Driver_plugin_callback_model.md) defines how the host exposes capabilities
  back to the plugin, including the dependency-inversion registry that lets the standard matcher/generator set be
  registered without the driver depending on the matching engine that implements it.
- [005 Plugin capability negotiation and versioning](./005_Plugin_capability_negotiation_and_versioning.md) is how a
  plugin discovers whether a given standard matcher/generator is available, via `hostCapabilities` at `InitPlugin`.

## Resolved questions

- **Which host matcher and generator capabilities should be exposed first?** The standard Pact matching rule and
  generator set, registered as `CORE` entries with handlers implementing 006's field-level trait shape. The entries
  themselves now exist in both hosts, one per rule and generator each implements
  (see [Registered entries](#registered-entries)); the handlers go on the subset that can act on a single value
  (see [Delegatable set](#delegatable-set)).
- **How is a standard rule named when a plugin calls back for it?** By the name it carries in the request it was
  handed - `type`, `content-type`, `not-empty` for matching rules, `RandomInt`, `DateTime` for generators - which is
  the catalogue entry key. The specification version the rule was introduced in is a `spec-version` value on the
  entry, not part of the key: a plugin should not have to know that `type` arrived in V2 and `semver` in V4 to ask
  for either. (The earlier design keyed entries `v2-type`/`v3-date` and had the drivers' resolver strip the prefix;
  that pass has been removed from both drivers.)
- **Should delegation be explicit in plugin responses, or should the host always be free to resolve standard rules
  itself?** Explicit. This follows from 007's model: a plugin calls a specific, named capability when it wants
  host-provided behaviour. The host never silently intercepts or overrides a plugin's own handling of a rule it chose
  to implement itself — that would violate 007's "typed capabilities, not a generic envelope, and the plugin always
  knows what it's calling" principle.
- **How should the host expose rule configuration, context, and mismatch results so they remain consistent with
  existing Pact behaviour?** By reusing the same types 006 and 007 already commit to reusing rather than defining a
  parallel model: `MatchingRule`/`Generator` (existing `oneof`-based values) for configuration, `ContentMismatch` for
  results, and the field-level context 006 defines for surrounding-document visibility.
- **How do we avoid tight coupling between plugins and one specific host implementation while still exposing useful
  host functionality?** This is exactly what 007's registered-handler mechanism solves: a plugin only ever sees a
  catalogue key and a typed request/response. Which concrete Pact framework implementation registered the handler
  behind that key is invisible to both the plugin and the driver's compiled code.
- **Do the collection-wide rules get handlers too?** No - `min-type`, `values`, `array-contains`, `each-key`,
  `each-value` and the `*ignore-order` family stay host-only, per 006's non-goals, and so does the `ArrayContains`
  generator. They keep their catalogue entries, since the host does have those capabilities and 005 discovery should
  see them, but a callback naming one gets an error saying so.

## Open questions

- **Should a callback for a non-delegatable rule be branchable?** It currently returns `MatchFieldResponse.error`
  with a message saying the rule applies to a collection as a whole. That is a bare string, so a plugin that wants
  to fall back to its own handling has to match on the text. A typed reason on the response would be better, but it
  is a proto change for a case no plugin has hit yet.
- **Do `ProviderState` and `MockServerURL` generators need anything beyond `testContext`?** Both are registered and
  both read what they need from the request's test context, so a caller that populates it gets the right answer -
  but nothing yet checks that every host path which applies a field generator *does* populate it. A plugin calling
  `ProviderState` from a verification where the context was not passed through would get a generation error rather
  than a wrong value, so this is a diagnosability question rather than a correctness one.
