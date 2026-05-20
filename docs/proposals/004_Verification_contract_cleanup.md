# Verification contract cleanup (Draft)

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

- Replace the “full pact JSON + interaction key” verification contract with a dedicated interaction-level verification model.
- Send the interaction data in a structured form that is independent of Pact JSON parsing.
- Keep the contract focused on the data required to prepare and execute verification, including:
  - request/response or message payload data;
  - transport and metadata fields;
  - plugin-specific persisted configuration for the interaction;
  - user-supplied verification configuration;
  - any context required to report verification results cleanly.
- Prefer an interface shape that can be mapped to both gRPC and future in-process plugin runtimes.

## Non-goals for this proposal

- Defining field-level matcher callbacks.
- Defining a generic callback or host-function protocol.
- Redesigning plugin packaging or installation.

## Open questions

- What is the smallest interaction model that still supports transport plugins cleanly?
- Which data belongs in the “prepare verification” step versus the “execute verification” step?
- How should plugin-specific persisted configuration be separated from generic interaction data?
