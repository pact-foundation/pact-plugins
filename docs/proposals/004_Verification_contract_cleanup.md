# Verification contract cleanup (Draft)

> [!NOTE]
> **Implementation phase:** Phase 1 (foundational). Deliver alongside [005](./005_Plugin_capability_negotiation_and_versioning.md) before any other V2 interface work begins. See the [proposals README](./README.md) for the full delivery order.

## Summary

Refine the plugin verification API so plugins receive the specific interaction data they need, instead of having to
re-parse a full Pact document and locate the target interaction themselves.

## Problem statement

The current verification flow still sends the full Pact as JSON together with an interaction key. That makes transport
plugins depend on Pact JSON parsing and Pact model knowledge, even when they only need the verification request data,
the plugin-specific interaction configuration, and a small amount of surrounding metadata.

This creates unnecessary coupling between plugin authors and the Pact data model, increases implementation complexity,
and makes the verification API harder to evolve.

## Recommended direction

### Retain the two-phase split

The existing split between `PrepareInteractionForVerification` and `VerifyInteraction` is correct and should be kept. The prepare step exists so that users can amend request data (for example, injecting auth tokens or overriding headers) before the request is executed. Collapsing the two phases would remove that opportunity.

### Replace pact-as-JSON with a structured interaction model

Replace the `pact + interactionKey` fields in both verification requests with a dedicated message carrying only the data the plugin actually needs:

- **Interaction type** — the V4 interaction type (synchronous HTTP, message, synchronous message, etc.) so the plugin knows how to interpret the payload without parsing a full Pact document.
- **Interaction-level plugin configuration** — the `interactionConfiguration` data persisted by the plugin during the consumer test (`PluginConfiguration.interactionConfiguration`). This is the plugin's own stored state and should be delivered directly rather than requiring the plugin to extract it from a Pact JSON tree.
- **Pact-level plugin configuration** — the `pactConfiguration` data persisted in the Pact file metadata (`PluginConfiguration.pactConfiguration`), for any global plugin state needed at verification time.
- **Consumer and provider names** — sufficient context for result reporting and log correlation without requiring the full Pact metadata.
- **User-supplied verification configuration** — already present as `config` in the current requests; retain this field.
- **Test context** — a `testContext` field carrying test-framework-supplied context. This field already exists in `GenerateContentRequest` and `StartMockServerRequest` but is absent from both verification requests; this inconsistency should be fixed here.

The existing `InteractionData` message (body + metadata map) is already the right shape for carrying request and response body data and should be reused in the request direction rather than introducing a new type.

### Apply the same fix to the mock server API

`StartMockServerRequest` has the same pact-as-JSON coupling. The fix should be applied consistently so that transport plugins do not need Pact parsing in any code path.

### Complete the deprecated mock server type cleanup

Replace the deprecated `ShutdownMockServerRequest` and `ShutdownMockServerResponse` types with `MockServerRequest` and `MockServerResults` respectively. These replacements already exist in the proto and the swap is marked as a TODO for the next major version.

## Non-goals for this proposal

- Defining field-level matcher callbacks.
- Defining a generic callback or host-function protocol.
- Redesigning plugin packaging or installation.
- Maintaining backwards compatibility with V1 plugins — that is handled by [005](./005_Plugin_capability_negotiation_and_versioning.md) via the manifest `pluginInterfaceVersion` field. The driver selects the old or new verification format based on that value; V1 plugins continue to receive the existing pact-as-JSON request unchanged.

## WASM compatibility

The structured interaction model defined by this proposal must be transport-neutral. The replacement for the pact-as-JSON approach must work equally as fields in a gRPC message (external process plugins) and as parameters to a host function call or WASM export (in-process WASM plugins). This means the model must not embed transport-specific assumptions or encoding.

The same pact-as-JSON problem exists in the mock server flow: `StartMockServerRequest` also passes the full Pact as a JSON string. Any solution defined here should be applied consistently to the mock server API to avoid leaving a parallel coupling in place.

## Open questions

- What is the smallest interaction model that still supports transport plugins cleanly? The fields listed above are candidates; implementation experience may show some can be dropped.
- Are consumer and provider names sufficient context, or does the plugin need additional Pact-level metadata (e.g. Pact specification version)?
- How should the structured interaction model be represented for the mock server case, where the plugin receives all interactions upfront rather than one at a time?
