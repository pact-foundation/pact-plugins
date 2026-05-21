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

- Define the logical callback model first, before choosing a concrete transport-specific implementation.
- Keep the model transport-agnostic so it can be mapped to:
  - gRPC-based external plugins;
  - in-process runtimes that expose host functions directly.
- Specify the lifecycle and failure semantics explicitly, including:
  - correlation and nesting rules;
  - deadlines and cancellation;
  - cycle detection or prevention;
  - error propagation;
  - behaviour when a callback target is unavailable.
- Prefer a narrow set of well-defined host capabilities over a generic “message bus”.

## Non-goals for this proposal

- Defining the detailed payload model for verification.
- Solving observability/logging by itself.
- Redesigning plugin discovery or packaging.

## WASM compatibility

The callback model must map to two fundamentally different transports:
- **gRPC plugins**: callbacks require either a reverse connection (the plugin connects back to a driver-side gRPC server) or bi-directional streaming. The tradeoffs between these approaches must be evaluated explicitly as part of this proposal.
- **WASM plugins**: callbacks are host-exported functions that the WASM module imports at load time. There is no network connection.

The primary deliverable of this proposal is the transport-neutral logical interface: what host capabilities can be called, what parameters they accept, what they return, and what the lifecycle and failure rules are. The gRPC and WASM transport mappings follow from that definition and should be treated as secondary concerns.

## Open questions

- Which host operations are worth exposing first?
- Is a generic callback envelope required, or should callbacks be modelled as specific capabilities?
- How should callback support differ between external process plugins and in-process runtimes?
