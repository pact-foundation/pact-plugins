# Consumer DSL / plugin config inconsistencies between pact-jvm and pact-reference

## Context

While adding support for pact plugins written in Lua (see
[`002_Support_script_language_plugins.md`](../proposals/002_Support_script_language_plugins.md))
to both the Rust (`drivers/rust/driver`) and JVM (`drivers/jvm/core`) drivers, a JWT example
plugin (`plugins/jwt/`) was used as a real end-to-end test in both `examples/jwt/jwt-consumer-rust`
and `examples/jwt/jwt-consumer-jvm`. That plugin uses the same content type
(`application/jwt+json`) for **both** the request and the response body of a single
interaction, which turned out to be an unusual enough combination that it exercised three
behavioral inconsistencies between the two driver/consumer-DSL stacks. One of them
(content-type catalogue matching) was fixed directly in this repo — see commit `c9d90af`
("fix(catalogue): make content-type matching semantics consistent across drivers"). The other
two, documented here, live in the **consumer DSL libraries** (`pact-jvm` and
`pact-reference`), not in `pact-plugins` itself, so they need their own fixes/PRs in those
repos. This doc exists so that work can be picked up later without re-deriving the
investigation.

The current workarounds for both issues below are already in `plugins/jwt/plugin.lua` (see
its comments) — they should be safe to leave in place even after the underlying libraries are
fixed, but could be simplified once fixed.

## Issue 1: `pact-reference`'s interaction-level plugin config doesn't merge `interaction_configuration` across request/response

**Symptom**: if one interaction configures its request body and response body with the same
plugin, and that plugin persists *different* `interaction_configuration` data for each part,
one of the two silently disappears when the pact is built with `pact-reference` (Rust). It
does not reproduce with `pact-jvm`.

**Root cause is a genuine bug, not a missing feature** — both libraries actually intend the
same "namespace by part" convention:

- `pact-jvm`: `HttpRequest.transformConfig`/`HttpResponse.transformConfig`
  (`core/model/src/main/kotlin/au/com/dius/pact/core/model/V4HttpParts.kt:103` and `:233`)
  wrap the plugin's returned `interactionConfiguration` under a `"request"` or `"response"`
  key respectively (the default `HttpPart.transformConfig`, `core/model/.../HttpPart.kt:46`,
  is identity — only HTTP request/response parts get namespaced; message-type interactions
  don't need it since they only ever have one part).
  `V4Pact.addPluginConfiguration` (`core/model/.../V4Pact.kt:199`) then **deep-merges**
  successive calls for the same plugin name, so calling it once for the request and once for
  the response correctly accumulates `{"request": {...}, "response": {...}}` in one place.
- `pact-reference`: `request_builder.rs` and `response_builder.rs`
  (`rust/pact_consumer/src/builders/`) **also** wrap the config under `"request"`/`"response"`
  keys (~lines 213–226 in each — search for `hashmap!{ "request".to_string() => ...` /
  `"response".to_string() => ...`). But the call that stores it,
  `self.plugin_config.insert(matcher.plugin_name(), plugin_config)` (`request_builder.rs:225`,
  same pattern in `response_builder.rs`), is a **plain `HashMap::insert`, which replaces any
  existing entry for that plugin name wholesale** rather than merging. A few lines further
  down (`request_builder.rs:232-241`) there *is* a merge-if-exists code path — but it only
  merges `pact_configuration`, not `interaction_configuration`. Net effect: whichever of
  request/response is configured **second** for a given plugin overwrites the first's
  `interaction_configuration` entirely.

This didn't surface in the JWT example because its request and response configs are
identical (same public key/algorithm), so the overwrite is invisible — the "wrong" value
happens to equal the "right" one.

**Suggested fix**: in `pact-reference`, give `interaction_configuration` the same
merge-if-key-exists treatment `pact_configuration` already gets in `request_builder.rs`/
`response_builder.rs` (and check `message_builder.rs`/`sync_message_builder.rs` too, though
those don't have a request/response split so may not need it).

## Issue 2: `"+json"`-suffixed content types are assumed to be parseable JSON when writing pact files, and only a `BINARY` content-type hint bypasses this — `TEXT` doesn't, and `pact-jvm` doesn't degrade gracefully like `pact-reference` does

**Symptom**: a body whose content type ends in a `+json` structured syntax suffix (RFC 6839 —
e.g. `application/jwt+json`, or `application/vnd.google.protobuf+json`) but whose actual
bytes are **not** parseable JSON (a compact JWT is `header.payload.signature`, not JSON)
crashes pact file serialization on `pact-jvm` with an uncaught `JsonException`, even when the
plugin has set the body's `contentTypeHint` to `TEXT`.

**Details**:
- `pact-jvm`: `ContentType.isJson()` (`core/model/.../ContentType.kt:19`) returns true for any
  type with a `+json` suffix (or subtype starting with `json`), *regardless of any content
  type hint*. `OptionalBody.toV4Format()` (`core/model/.../OptionalBody.kt:169`) checks
  `isJson()` first; it only skips the JSON-parse attempt if `contentTypeHint == BINARY`.
  Otherwise it calls `JsonParser.parseString(valueAsString())`, which **throws** on non-JSON
  content — an uncaught exception that fails the entire pact-file write, not just that one
  interaction.
- `pact-reference`: `ContentType::is_json()` (`rust/pact_models/src/content_types.rs:93`) has
  the same "+json suffix means JSON" logic. `OptionalBody::to_v4_json()`
  (`rust/pact_models/src/bodies.rs:140`) does the **same initial check** (`is_json()` first,
  ignoring the hint) and **attempts** the same JSON parse — but it **catches the failure**
  and gracefully falls back to base64-encoding the body with a `warn!` log, rather than
  panicking or propagating an error. So `pact-reference` never crashes here regardless of
  hint, though a non-`BINARY`-hinted non-JSON `+json` body still gets needlessly (if
  harmlessly) base64-encoded in the persisted pact file instead of kept as a readable plain
  string.

**Current workaround** (`plugins/jwt/plugin.lua`): the JWT body is hinted `content_type_hint
= "BINARY"` rather than `"TEXT"`, which correctly short-circuits the JSON-parse attempt on
both drivers.

**Suggested fixes**:
1. `pact-jvm`: catch the `JsonException` in `OptionalBody.toV4Format()` and fall back to
   base64 (matching `pact-reference`'s existing graceful-degradation behavior), instead of
   letting it propagate and fail the whole pact-file write.
2. Both `pact-jvm` and `pact-reference`: consider skipping the "attempt-JSON-parse" path
   entirely when `contentTypeHint == TEXT`, not just `BINARY` — currently *neither* honors an
   explicit `TEXT` hint for `+json`-suffixed types, which is surprising: if a plugin author
   affirmatively says "treat this as plain text," a `+json` suffix shouldn't override that.

## Suggested next steps

- Track these as issues in `pact-foundation/pact-jvm` and `pact-foundation/pact-reference`
  respectively (separate release trains from `pact-plugins`).
- Issue 1 fix: `rust/pact_consumer/src/builders/request_builder.rs` and `response_builder.rs`.
- Issue 2 fix: `pact-jvm`'s `core/model/src/main/kotlin/au/com/dius/pact/core/model/OptionalBody.kt`
  (primary crash fix); consider a matching hint-handling change in both repos' `is_json()`/
  `toV4Format()`/`to_v4_json()` equivalents.
- Once fixed, `plugins/jwt/plugin.lua`'s workarounds (the `BINARY` hint, and reading both
  `interaction_configuration.request`/`.response`/flat) can likely stay as-is for backward
  compatibility with older consumer-DSL versions, but could be simplified.
