# Host-provided core matching and generation (Draft)

> [!NOTE]
> **Implementation phase:** Phase 4. Cannot be implemented until [005](./005_Plugin_capability_negotiation_and_versioning.md), [006](./006_Field_level_matchers_and_generators.md), and [007](./007_Driver_plugin_callback_model.md) are all finalised. See the [proposals README](./README.md) for the full delivery order.

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
already defines, naming the catalogue key for the standard rule it wants (e.g. `matcher/type`, `matcher/regex`).

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
   `PluginHostServer.kt`) — see [007](./007_Driver_plugin_callback_model.md#sequencing). This unblocks registering
   any *content-level* standard matcher/generator (a whole content type, not a single field) through the mechanism
   today.
2. ⬜ [006](./006_Field_level_matchers_and_generators.md)'s field-level operation shape. Not started. Blocks
   registering the field-level standard rules (`type`, `regex`, `equality`, etc. applied to a single field rather
   than a whole content type), which is most of what this proposal is actually for.
3. ⬜ Register the standard Pact matcher/generator set as `CORE` catalogue entries with handlers implementing 007's
   traits (content-level now possible; field-level blocked on step 2). Not started.
4. ⬜ WASM and Lua host-function equivalents, following [007](./007_Driver_plugin_callback_model.md#sequencing)'s
   step 4. Not started.

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
  generator types already represented in the core catalogue (`type`, `regex`, `equality`, `include`, `date`, `time`,
  `datetime`, etc.), registered as `CORE` entries with handlers implementing 006's field-level trait shape.
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
