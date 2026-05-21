# Field-level matchers and generators (Draft)

> [!NOTE]
> **Implementation phase:** Phase 3 (new functionality). Requires [005](./005_Plugin_capability_negotiation_and_versioning.md) to be finalised. Design in parallel with [007](./007_Driver_plugin_callback_model.md); data model decisions in these two proposals must be kept consistent. See the [proposals README](./README.md) for the full delivery order.

## Summary

Add a plugin API for matching and generating data at the field or element level, instead of limiting plugins to whole
content types and transport-level interactions.

## Problem statement

The catalogue model already recognises matcher entries, but the current runtime interface is focused on content
matchers/generators and transport plugins. This leaves a gap for use cases where a plugin should contribute matching or
generation logic for a specific field, key, header, token, or nested value.

Without an explicit field-level API, plugin authors either cannot express these use cases at all or are pushed into
whole-content plugins that are broader and more complex than the problem requires.

## Recommended direction

- Define dedicated field-level matcher and generator operations rather than overloading whole-content APIs.
- Keep the API binary-safe and context-aware:
  - do not assume all values can be represented as JSON;
  - include the matching path or location being evaluated;
  - include the selected matcher/generator entry and any associated values;
  - include plugin configuration and any mode/context needed for generation.
- Align the mismatch/result model with existing Pact mismatch reporting so results can be surfaced consistently across
  drivers and UIs.

## Non-goals for this proposal

- Redesigning whole-content matcher/generator flows.
- Defining a general-purpose callback bus between plugins and the host.
- Solving plugin runtime/version negotiation on its own.

## WASM compatibility

For gRPC plugins, field-level matching and generation operations will be new RPCs in the plugin service. For WASM plugins, the same operations will be exported WASM functions that the host calls into. The data types used must be representable in both forms.

In practice this means:
- value types should follow the existing `oneof` pattern (see `MetadataValue` in the current proto) rather than assuming JSON encoding;
- path expressions should use the existing Pact matching rule expression format already used throughout the interface;
- mismatch results should reuse the existing `ContentMismatch` type rather than introducing a parallel structure.

## Open questions

- What value model should be used for binary-safe field-level matching and generation?
- How much of the surrounding document context should be visible to a field-level plugin call?
- Should field-level generators be pure functions, or can they depend on host-provided context?
