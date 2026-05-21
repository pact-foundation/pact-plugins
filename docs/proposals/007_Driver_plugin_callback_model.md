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

## Recommended direction

### Logical model

Callbacks are modelled as specific typed host capabilities, not a generic message envelope. Each capability is a well-defined operation with a typed request and response. The driver advertises which capabilities it supports during the `InitPlugin` handshake (via [005](./005_Plugin_capability_negotiation_and_versioning.md)), so a plugin knows before making any call whether a given capability is available.

### gRPC transport

The driver implements a `PactPluginHost` gRPC service on a local listener and passes its address to the plugin in `InitPlugin`. The plugin creates a standard gRPC client channel to that address. Each callback is a normal blocking unary RPC call from plugin to driver — no bi-directional streaming is required.

A concrete flow during `VerifyInteraction`:

```
Driver ──── VerifyInteraction(request) ────────────────→ Plugin
                                                         processes...
Plugin ──── MatchField(request) ───────────────────────→ PactPluginHost (driver)
Plugin ←─── MatchFieldResult ──────────────────────────── driver
                                                         continues...
Driver ←─── VerifyInteractionResponse ─────────────────── Plugin
```

Each callback completes before the plugin continues. The call stack is synchronous and nested, which makes error propagation and deadline tracking straightforward.

**Cycle detection is required for gRPC.** A call chain ID must be threaded through gRPC request metadata for any request that may trigger callbacks. If the driver receives a callback whose chain ID matches an in-flight request it is currently processing, the call is a cycle and must be rejected with a clear error. The plugin surfaces this as a failure in the parent request.

**Deadlines:** a callback's deadline must be bounded by the remaining deadline of the parent request that triggered it. The driver enforces this when it receives the callback.

**Unavailable target:** if the driver's `PactPluginHost` service is unreachable, the plugin must fail the parent request with a clear error rather than hanging.

### WASM transport

Host capabilities are exposed as host-exported functions that the WASM module imports at load time. Calls resolve via the native call stack — there is no network hop and no blocking concern. A true cycle would manifest as a stack overflow, which the WASM runtime handles. No explicit cycle detection is needed for WASM.

## Non-goals for this proposal

- Defining the detailed payload model for verification.
- Solving observability/logging by itself.
- Redesigning plugin discovery or packaging.

## WASM compatibility

For WASM plugins, the callback model maps directly to host-exported functions imported by the module at load time. This is the established model for WASM host integration and works well: calls are synchronous, resolve via the call stack, and require no connection management or cycle detection logic in the plugin or driver.

The logical capability interface — what can be called, what parameters it takes, what it returns — is identical between gRPC and WASM. Only the transport differs.

## Open questions

- Which specific host capabilities should be exposed first? This will be driven by the needs of [006](./006_Field_level_matchers_and_generators.md) and [009](./009_Host_provided_core_matching_and_generation.md).
