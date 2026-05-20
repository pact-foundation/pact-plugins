# Host-provided core matching and generation (Draft)

## Summary

Allow plugins to delegate standard Pact matching and generation behaviour back to the host framework, instead of having
to embed or re-implement that logic themselves.

## Problem statement

Plugin authors currently need to reproduce Pact matching and generation logic that already exists in the host runtime.
In practice, this can force a plugin to depend on a host implementation such as Pact-JVM just to access standard Pact
matchers, generators, and related data model behaviour.

That creates unnecessary duplication, increases plugin complexity, and weakens the portability story for plugins. It
also means plugins can drift from the behaviour of the host they are running under, even when the host already has the
correct implementation of the standard Pact rules.

This becomes particularly important for plugins that work at the field or element level, where the plugin may only need
specialised handling for a subset of the data and should be able to delegate the rest to the host.

## Recommended direction

- Treat standard Pact matching and generation as host capabilities that plugins can call into when required.
- Build this on top of:
  - a field-level plugin API, so plugins can hand off or compose matching/generation at a smaller granularity;
  - a callback/host-function model, so the host can expose existing Pact functionality without the plugin embedding a
    specific Pact implementation.
- Start with the existing standard Pact matcher and generator set already represented in the core catalogue.
- Define delegation in a way that preserves host behaviour as the source of truth for standard Pact semantics.
- Keep the interface transport-agnostic so the same capability can be exposed to:
  - external gRPC plugins;
  - in-process runtimes such as script or WASM plugins.

## Non-goals for this proposal

- Defining the generic callback protocol on its own.
- Defining new field-level matcher/generator operations from scratch.
- Replacing all plugin logic with host-side logic; plugins should still own their specialised behaviour.

## Relationship to other proposals

- [006 Field-level matchers and generators](./006_Field_level_matchers_and_generators.md) defines where plugin-owned
  matching and generation hooks are needed.
- [007 Driver-plugin callback model](./007_Driver_plugin_callback_model.md) defines how the host can expose
  capabilities back to the plugin.
- [005 Plugin capability negotiation and versioning](./005_Plugin_capability_negotiation_and_versioning.md) is likely
  required so plugins can discover whether host-provided matching/generation is available.

## Open questions

- Which host matcher and generator capabilities should be exposed first?
- Should delegation be explicit in plugin responses, or should the host always be free to resolve standard rules itself?
- How should the host expose rule configuration, context, and mismatch results so they remain consistent with existing
  Pact behaviour?
- How do we avoid tight coupling between plugins and one specific host implementation while still exposing useful host
  functionality?
