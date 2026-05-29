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

### Manifest versus handshake

Capabilities live at two levels with distinct roles:

- **Manifest (`pluginInterfaceVersion`)** — coarse version gate, read by the driver before the plugin process starts. Determines which protocol the driver uses for all subsequent calls. No capability negotiation is needed for V1 plugins; the driver simply falls back to the existing protocol.
- **`InitPlugin` handshake** — fine-grained, per-feature negotiation between a running V2 plugin and the driver. Both sides declare their optional capabilities here; the result determines which V2 features are active for the lifetime of this plugin instance.

### Negotiation is bidirectional

The plugin declares to the driver which optional capabilities it supports. The driver must also advertise to the plugin which host capabilities it exposes (for example, host-provided matching from [009](./009_Host_provided_core_matching_and_generation.md)). Both directions flow through the `InitPlugin` handshake.

### Compatibility rules

- A plugin declaring a capability the driver does not recognise is silently ignored. The driver continues with the capabilities it does understand.
- A plugin that requires a host capability the driver does not provide must fail startup with a clear error message. Proceeding without a declared required capability would produce incorrect behaviour that is harder to diagnose than an explicit failure.

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

- Which specific capabilities should be mandatory versus optional? The compatibility rules above define how each category is handled; the open question is which capabilities belong in which category as they are defined in later proposals.

## Initial capability set for Phase 1

Phase 1 needs one real capability pair so the negotiation path is exercised end to end before later proposals add more
behaviour. The first capability set is intentionally small and based on behaviour the current drivers and CSV plugin
already rely on.

- **Host capability: `host/interaction/request-response`**
  - Meaning: the driver provides request/response-scoped interaction sections to V2 plugins where appropriate, instead
    of flattening everything into one unscoped interaction block.
  - Why first: the local CSV V2 plugin already relies on this shape for request-body matching and generation.
- **Plugin capability: `plugin/interaction/request-response`**
  - Meaning: the plugin understands request/response-scoped interaction sections and can safely consume them for its
    V2 interaction and content APIs.
  - Why first: it is a genuine plugin-side optional feature that can be exercised today without waiting for the later
    callback proposals.

For this initial phase, the CSV plugin should require `host/interaction/request-response` during `InitPlugin`
startup. That gives us a concrete failure mode if a V2 driver does not advertise the capability it actually needs to
provide.
