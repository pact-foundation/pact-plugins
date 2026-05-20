# Plugin capability negotiation and versioning (Draft)

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
  - possible in-process runtimes such as script or WASM plugins.

## Non-goals for this proposal

- Defining the detailed verification payload model.
- Defining host callbacks or field-level matcher RPCs.
- Changing plugin discovery or repository index behaviour.

## Open questions

- Which capabilities should be mandatory versus optional?
- Should capabilities live in the startup handshake, the manifest, or both?
- How should drivers surface partial support or unsupported capability combinations to users?
