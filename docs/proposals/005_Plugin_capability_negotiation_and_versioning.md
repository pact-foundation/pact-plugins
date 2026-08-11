# Plugin capability negotiation and versioning

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

## Capability categories

Capabilities fall into two categories:

- **Baseline V2**: Every correctly-implementing V2 driver MUST advertise these. A plugin that requires a baseline capability can safely fail startup if the driver does not declare it, because any conformant V2 driver will. A V2 driver that omits a baseline capability is broken, not just limited.
- **Optional**: A V2 driver may or may not provide these. Plugins that need an optional capability should either fail gracefully with a clear error, or operate in a degraded mode without it.

The rule for classifying a new capability: it is **baseline** if its absence would cause silently incorrect behaviour in any V2 interaction (i.e., it describes the protocol structure itself). It is **optional** if its absence means a feature is unavailable but the protocol still operates correctly.

Future proposals must state which category each new capability belongs to.

## Initial capability set for Phase 1

Phase 1 needs one real capability set so the negotiation path is exercised end to end, in both directions, before
later proposals add more behaviour. The first set is the interaction types themselves: there are three interaction
types a Pact file can record, so there are three capabilities, one per type.

| Capability | Interaction type | V4 type string |
| --- | --- | --- |
| `interaction/request-response` | Synchronous request/response | `Synchronous/HTTP` |
| `interaction/message` | Asynchronous messages (one off or fire and forget) | `Asynchronous/Messages` |
| `interaction/synchronous-message` | Synchronous messages (request/response, like gRPC) | `Synchronous/Messages` |

The transport an interaction is carried over is a separate concern, covered by the `transport/*` catalogue entries -
it is only history that the request/response interaction is the original Pact one carried over HTTP or HTTPS. A
plugin that provides a transport says so with a `TRANSPORT` catalogue entry, independently of which interaction
types it can handle.

- **As host capabilities** they mean: the host can record interactions of this type in a Pact file, and so may hand
  the plugin one. Each host registers them as `INTERACTION` catalogue entries, which is what `hostCapabilities` is
  derived from.
- **As plugin capabilities** they mean: the plugin understands interactions of this type and can consume them in its
  V2 interaction, verification and content APIs.

These are **optional** capabilities under the classification above, in both directions: a plugin that declares none
still works, and a host that carries only some interaction types is limited rather than broken.

## How the negotiated capabilities are used

Both drivers check a plugin's declared interaction capabilities before handing it an interaction for verification -
`prepare_validation_for_interaction` and `verify_interaction` in the Rust driver, `prepareValidationForInteraction`
and `verifyInteraction` on the JVM. If the plugin is a V2 plugin that declared at least one interaction capability
and the interaction's type is not among them, the driver fails with an error naming the plugin, the interaction type
and the missing capability, rather than sending the interaction and letting the plugin fail further in with a less
obvious error.

A V2 plugin that declares no interaction capability at all is treated as supporting every type. That keeps V2
plugins written before these capabilities existed working unchanged; declaring one is opting in to the check for all
three.

The verification calls are the right place because they are per-interaction. `ConfigureInteraction` is dispatched by
content type and carries no interaction type at all, so there is nothing there to check - a plugin's content-matching
capabilities are already scoped by its `CONTENT_MATCHER`/`CONTENT_GENERATOR` catalogue entries. `StartMockServer`
carries whole-Pact interaction contents, and the Pact handed to a transport plugin's mock server can legitimately
contain interactions belonging to other transports (see the `http-plus-grpc` example), so an interaction type the
plugin did not declare is not an error there.

Reference plugins in this repository declare no interaction capabilities yet, and so are unaffected. A plugin opts in
when it wants the driver to enforce the boundary on its behalf.
