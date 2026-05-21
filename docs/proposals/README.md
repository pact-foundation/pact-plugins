## Pact Plugin Design Proposals

Here is the current list of proposed changes to the Pact Plugin architecture. Proposals 004–009 are listed in delivery order rather than numerical order.

| Proposal | Phase | State | Discussion |
|---|---|---|---|
| [V2 Plugin Interface](./001_V2_Plugin_Interface.md) | Historical | Superseded | https://github.com/pact-foundation/pact-plugins/discussions/83 |
| [Support script language plugins](./002_Support_script_language_plugins.md) | Not proceeding | Draft | https://github.com/pact-foundation/pact-plugins/discussions/84 |
| [Support WASM plugins](./003_Support_WASM_plugins.md) | Background | Draft | https://github.com/pact-foundation/pact-plugins/discussions/85 |
| [Verification contract cleanup](./004_Verification_contract_cleanup.md) | Phase 1 | Draft | TBD |
| [Plugin capability negotiation and versioning](./005_Plugin_capability_negotiation_and_versioning.md) | Phase 1 | Draft | TBD |
| [Plugin observability and logging](./008_Plugin_observability_and_logging.md) | Phase 2 | Draft | TBD |
| [Field-level matchers and generators](./006_Field_level_matchers_and_generators.md) | Phase 3 | Draft | TBD |
| [Driver-plugin callback model](./007_Driver_plugin_callback_model.md) | Phase 3 | Draft | TBD |
| [Host-provided core matching and generation](./009_Host_provided_core_matching_and_generation.md) | Phase 4 | Draft | TBD |

## Implementation phases

**Phase 1 — Foundational (004, 005)**
These two proposals are independent of each other and establish the groundwork for all later work. 004 cleans up the verification contract so plugins no longer need to parse full Pact JSON. 005 introduces explicit capability negotiation so new interface features can be adopted incrementally without an all-or-nothing version bump. Both must ship before any Phase 2 or later work begins.

**Phase 2 — Observability (008)**
008 addresses a real and immediate user pain point — plugin logging is currently difficult to work with. It is largely independent of the new functionality proposals and should be delivered after Phase 1 but before the more complex Phase 3 work, so that diagnostic improvements are in place before the interface grows more complex.

**Phase 3 — New functionality (006, 007)**
006 and 007 can be designed in parallel. 007 (the callback model) must define its logical interface first; the concrete gRPC and WASM transport mappings follow from that definition. 006 (field-level matchers and generators) aligns with 007's data model but does not depend on it being fully implemented first.

**Phase 4 — Host-provided matching (009)**
009 depends on 005, 006, and 007 all being finalised and cannot ship before them.

## WASM compatibility

[Proposal 003](./003_Support_WASM_plugins.md) (WASM-based plugins) is in scope alongside the V2 interface work. Proposal 002 (script language plugins) is not being pursued.

All proposals in this set must treat the interface as a transport-neutral logical contract. Every design decision should be validated against both runtime models:
- **gRPC plugins** run as external processes and communicate over a network connection; new operations become new RPCs.
- **WASM plugins** run in-process; new operations become exported or imported WASM functions or host function calls.

Concrete examples in the proposals may use gRPC for illustration, but the logical interface must not embed transport-specific assumptions.
