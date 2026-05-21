# Plugin capability negotiation and versioning (Draft)

> [!NOTE]
> **Implementation phase:** Phase 1 (foundational). Deliver alongside [004](./004_Verification_contract_cleanup.md) before any other V2 interface work begins. See the [proposals README](./README.md) for the full delivery order.

## Summary

Define a clearer compatibility and capability model for plugins and drivers so new interface features can be introduced
incrementally without forcing an all-or-nothing interface version bump.

## Problem statement

Today, plugin compatibility is effectively driven by the manifest’s interface version and by whether a given RPC exists
in a particular implementation. That makes it difficult to add optional behaviour in a controlled way, and it leaves too
much ambiguity around what a driver can expect from a plugin beyond “it speaks version N”.

As the plugin system grows, this becomes a blocker for evolving the interface safely across multiple drivers, runtimes,
and plugin implementations.

## Recommended direction

- Introduce explicit capability negotiation during plugin initialisation.
- Separate the concepts of:
  - interface version;
  - optional capabilities;
  - transport/runtime-specific features.
- Make new functionality opt-in through declared capabilities rather than assuming every plugin on a given interface
  version supports every feature.
- Ensure the negotiated model works across:
  - external gRPC plugins;
  - in-process WASM plugins (see [003](./003_Support_WASM_plugins.md)).

### Backwards compatibility requirement

The host must support both V1 and V2 plugins simultaneously. Introducing a new interface version must not break existing plugins. This is a hard requirement, not a follow-up concern.

The manifest already carries a `pluginInterfaceVersion` field (currently `1` for all existing plugins). The driver reads the manifest before starting the plugin process, so the interface version is known before any RPC is made. This is the correct detection point:
- `pluginInterfaceVersion: 1` — driver uses the existing V1 protocol throughout; no capability negotiation.
- `pluginInterfaceVersion: 2` — driver uses the V2 protocol; capability negotiation occurs in the `InitPlugin` handshake.

V1 plugins require no changes. The driver carries the compatibility burden by maintaining both code paths.

## Non-goals for this proposal

- Defining the detailed verification payload model.
- Defining host callbacks or field-level matcher RPCs.
- Changing plugin discovery or repository index behaviour.

## WASM compatibility

Capability negotiation must work for both runtime models:
- For gRPC plugins, negotiation occurs in the `InitPlugin` RPC handshake.
- For WASM plugins, the equivalent is an init host function call at module load time.

The capability model — what capabilities exist, what declaring them means, and how the driver acts on them — must be defined at the logical level, independent of transport.

Negotiation is bidirectional: the plugin declares what it supports, and the driver must also advertise which host capabilities it exposes (for example, host-provided matching from [009](./009_Host_provided_core_matching_and_generation.md)). This is particularly important for WASM plugins, which cannot link to native Pact libraries and may depend entirely on host-provided capabilities to function.

## Open questions

- Which capabilities should be mandatory versus optional?
- Should capabilities live in the startup handshake, the manifest, or both?
- How should drivers surface partial support or unsupported capability combinations to users?
