# Driver-plugin callback model (Draft)

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

## Open questions

- Which host operations are worth exposing first?
- Is a generic callback envelope required, or should callbacks be modelled as specific capabilities?
- How should callback support differ between external process plugins and in-process runtimes?
