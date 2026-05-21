# Plugin observability and logging (Draft)

> [!NOTE]
> **Implementation phase:** Phase 2 (observability). Deliver after Phase 1 ([004](./004_Verification_contract_cleanup.md), [005](./005_Plugin_capability_negotiation_and_versioning.md)) but before the new functionality proposals in Phase 3. See the [proposals README](./README.md) for the full delivery order.

## Summary

Improve plugin diagnostics so plugin execution, failures, and logs are easier to understand across drivers, runtimes,
and test frameworks.

## Problem statement

Current plugin logging is difficult to work with, especially for external process plugins. Forwarding plugin stdout and
stderr into driver logs can be noisy, can mix host and plugin logs together, and makes it hard to trace the logs for a
single plugin instance or a single test run.

As the plugin system becomes more capable, diagnostics need to be designed deliberately rather than treated as a side
effect of process stdout.

## Recommended direction

- Define observability requirements as part of the plugin interface work, not as an afterthought.
- Prefer per-run diagnostic output with clear correlation to:
  - the plugin instance;
  - the driver/framework process;
  - the test or verification execution where possible.
- Distinguish between:
  - structured startup/handshake output;
  - operational logs;
  - user-facing verification/output messages.
- Ensure the design works for both external gRPC plugins and in-process runtimes.

## Non-goals for this proposal

- Defining the plugin callback protocol.
- Redesigning verification payloads or field-level matcher APIs.
- Replacing the existing plugin manifest format.

## WASM compatibility

WASM plugins do not have access to stdout or stderr by default. WASI provides optional I/O but it cannot be relied upon as a universal mechanism. Any logging strategy that depends on capturing process output does not apply to WASM plugins.

Structured log forwarding via a host function call must therefore be the primary logging mechanism, so that the same approach works for both gRPC and WASM plugins. For gRPC plugins this can be complemented by file-based output, but that cannot be the only path.

Delivering this before the Phase 3 functionality proposals ensures that field-level matchers, generators, and callbacks all have a consistent logging path from the start rather than retrofitting observability later.

## Open questions

- Should drivers capture plugin logs to per-run files, stream them through structured logging, or both?
- What minimum structured diagnostics should every plugin provide?
- How should user-facing plugin output be separated from low-level debug logs?
