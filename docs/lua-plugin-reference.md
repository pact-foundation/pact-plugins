# Lua plugin function reference

This is a field-by-field reference for the global functions, host functions, and table shapes involved in writing a
Pact plugin in Lua. For the narrative walkthrough (manifest setup, LuaRocks, logging, a worked example) see
[Writing plugins in Lua](writing-plugin-guide.md#writing-plugins-in-lua) in the plugin writing guide. This document
assumes you've read that first, and exists purely so you don't have to go spelunking through driver source to find
the exact key name or casing of a table field.

Both drivers (Rust and JVM) convert between these Lua tables and the same underlying gRPC message types a
compiled/`exec` plugin would send and receive, so the shapes are identical on either driver.

## Conventions

- **Table keys are case-sensitive Lua strings.** The overwhelming majority are `snake_case`. There are two
  deliberate exceptions, both called out below where they occur:
  - `entryType` (in a catalogue entry table) is `camelCase`, not `entry_type`.
  - `type-mismatch` (in a `match_contents` response) is hyphenated, not `type_mismatch`.
- **Enum-like string values are `SCREAMING_CASE` or `PascalCase`,** matching the underlying protobuf enum name
  exactly - see each function's description for the exact allowed values (e.g. `content_type_hint` is one of
  `"DEFAULT"`/`"TEXT"`/`"BINARY"`; `entryType` is one of `"CONTENT_MATCHER"`/`"CONTENT_GENERATOR"`/`"TRANSPORT"`/
  `"MATCHER"`/`"INTERACTION"`; `generate_content`'s `test_mode` is `"Consumer"`/`"Provider"`/`"Unknown"`).
- **A missing/absent table field is equivalent to Lua `nil`** unless stated otherwise - you don't need to set keys
  you have nothing to put in.
- **V1 vs V2 requests.** `pluginInterfaceVersion` in your `pact-plugin.json` manifest (1 or 2) is a static,
  per-plugin-instance choice - the driver never mixes the two for one plugin instance. It only affects the three
  transport functions that deal with a whole interaction/Pact (`start_mock_server`, `prepare_interaction_for_verification`,
  `verify_interaction`); a V2 request replaces the "whole Pact as a JSON string plus an interaction key" fields
  with a single structured `interaction_contents` table (see [`InteractionContents`](#interactioncontents-table-v2-transport-only)),
  and adds a `test_context` field. Both shapes are documented inline below.

## Global functions your script may define

| Function | Required? | Called for |
|---|---|---|
| [`init(implementation, version)`](#initimplementation-version---table) | Yes | Every plugin |
| [`configure_interaction(content_type, config)`](#configure_interactioncontent_type-config---table) | Yes, if you register a `CONTENT_MATCHER`/`CONTENT_GENERATOR` entry | Content-matcher plugins |
| [`match_contents(request)`](#match_contentsrequest---table) | Yes, if you register a `CONTENT_MATCHER` entry | Content-matcher plugins |
| [`generate_content(contents, generators, test_mode)`](#generate_contentcontents-generators-test_mode---table-optional) | No (passthrough default) | Content-generator plugins |
| [`update_catalogue(catalogue)`](#update_cataloguecatalogue-optional) | No (no-op default) | Every plugin |
| [`start_mock_server(request)`](#start_mock_serverrequest---table) | Yes, if you register a `TRANSPORT` entry | Transport plugins |
| [`shutdown_mock_server(server_key)`](#shutdown_mock_serverserver_key---table-and-get_mock_server_resultsserver_key---table) | Yes, if you register a `TRANSPORT` entry | Transport plugins |
| [`get_mock_server_results(server_key)`](#shutdown_mock_serverserver_key---table-and-get_mock_server_resultsserver_key---table) | Yes, if you register a `TRANSPORT` entry | Transport plugins |
| [`prepare_interaction_for_verification(request)`](#prepare_interaction_for_verificationrequest---table) | Yes, if you register a `TRANSPORT` entry | Transport plugins |
| [`verify_interaction(request)`](#verify_interactionrequest---table) | Yes, if you register a `TRANSPORT` entry | Transport plugins |

A plugin can register both a `CONTENT_MATCHER`/`CONTENT_GENERATOR` entry and a `TRANSPORT` entry from the same
`init` call, in which case it must define all the functions both roles require.

---

### `init(implementation, version) -> table`

Called once, immediately after your script is loaded (before any other function).

**Parameters**

| Name | Type | Description |
|---|---|---|
| `implementation` | string | Name of the calling framework/implementation (e.g. `"pact-jvm"`). |
| `version` | string | Version of the calling framework. |

**Return value**: an array (sequence) of catalogue entry tables:

| Field | Type | Required | Description |
|---|---|---|---|
| `entryType` | string | Yes | One of `"CONTENT_MATCHER"`, `"CONTENT_GENERATOR"`, `"TRANSPORT"`, `"MATCHER"`, `"INTERACTION"`. Note the `camelCase` - this is the one exception to the snake_case convention. |
| `key` | string | Yes | Your plugin's catalogue key for this entry (typically your plugin name). |
| `values` | table (string -> string) | No | Free-form metadata. For a `CONTENT_MATCHER`/`CONTENT_GENERATOR` entry, convention is a `content-types` key whose value is a semicolon-separated list of MIME types you handle, each one matched as a regex **anchored at both ends** against an actual content type - escape any regex metacharacter (most commonly `+`, as in a `+json` structured syntax suffix) for a literal match. |

```lua
function init(implementation, version)
  return {
    { entryType = "CONTENT_MATCHER", key = "jwt", values = { ["content-types"] = "application/jwt;application/jwt\\+json" } },
    { entryType = "CONTENT_GENERATOR", key = "jwt", values = { ["content-types"] = "application/jwt;application/jwt\\+json" } }
  }
end
```

---

### `configure_interaction(content_type, config) -> table`

Called once per interaction part (request or response) when a consumer test configures your content type.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `content_type` | string | The content type the test configured (e.g. `"application/jwt+json"`). |
| `config` | table or nil | The data the user supplied in their test, as a plain Lua table (JSON-shaped: nested tables/arrays/strings/numbers/booleans). |

**Return value**

| Field | Type | Required | Description |
|---|---|---|---|
| `interactions` | array of tables | Yes | One entry per interaction part produced (usually one). Each entry: see below. |
| `plugin_config` | table or nil | No | Global (Pact-level) plugin config - see [`PluginConfiguration`](#pluginconfiguration-table). |

Each entry in `interactions`:

| Field | Type | Required | Description |
|---|---|---|---|
| `contents` | table or nil | No | A [`Body`](#body-table). |
| `part_name` | string | No | Which part this is for (e.g. `"request"`/`"response"`), or `""` for a plain body. |
| `plugin_config` | table or nil | No | Interaction-level plugin config - see [`PluginConfiguration`](#pluginconfiguration-table). This is handed back to you as `plugin_configuration` in later `match_contents` calls for the same interaction. |

```lua
function configure_interaction(content_type, config)
  return {
    interactions = {
      {
        contents = { contents = "signed-token-string", content_type = "application/jwt+json", content_type_hint = "BINARY" },
        part_name = "",
        plugin_config = { interaction_configuration = { ["public-key"] = "...", algorithm = "RS512" } }
      }
    },
    plugin_config = { interaction_configuration = { ["public-key"] = "...", algorithm = "RS512" } }
  }
end
```

---

### `match_contents(request) -> table`

Called to compare actual content against expected content.

**Parameters** (`request`)

| Field | Type | Description |
|---|---|---|
| `expected` | table or nil | A [`Body`](#body-table). |
| `actual` | table or nil | A [`Body`](#body-table). |
| `allow_unexpected_keys` | boolean | Whether unexpected keys/fields in `actual` should be tolerated. |
| `rules` | table (path -> array of rule tables) | See [`MatchingRules`](#matchingrules-table). |
| `plugin_configuration` | table or nil | Whatever you returned as `plugin_config` from `configure_interaction` (see [`PluginConfiguration`](#pluginconfiguration-table)). |

**Return value**: exactly one of the following shapes.

- `{ error = "..." }` - a hard error; the match attempt itself failed (not a mismatch).
- `{ ["type-mismatch"] = { expected = "...", actual = "..." } }` - the content types themselves didn't match.
  Note the hyphenated key `type-mismatch`, the other convention-breaking key alongside `entryType`.
- `{ mismatches = { ["$"] = {...}, ["some.path"] = {...} } }` - a table keyed by matching-rule-expression path.
  Each value is one of:
  - a plain string (the mismatch description);
  - a [`ContentMismatch`](#contentmismatch-table) table;
  - an array mixing either of the above.
  An empty (or absent) `mismatches` table means the content matched.

---

### `generate_content(contents, generators, test_mode) -> table` (optional)

Called to generate contents using any defined generators. If you don't define this function, the driver passes
`contents` straight through unchanged.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `contents` | table or nil | A [`Body`](#body-table) - the content before generation. |
| `generators` | table (path -> generator table) | Each value: `{ type = "...", values = {...} }` - a [`Generator`](#generator-table). |
| `test_mode` | string | One of `"Consumer"`, `"Provider"`, `"Unknown"`. |

**Return value**: a [`Body`](#body-table), or `nil`.

---

### `update_catalogue(catalogue)` (optional)

Called whenever another plugin loads and the combined catalogue changes. If you don't define this function, it's a
no-op. No return value is used.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `catalogue` | array of tables | Each shaped like an `init` catalogue entry: `{ entryType, key, values }` (see [`init`](#initimplementation-version---table)). |

---

### `start_mock_server(request) -> table`

Called to start a mock server for a consumer test. **Your script is responsible for actually standing up whatever
listener the transport needs** - the driver only calls this function at the right point in the test lifecycle and
relays back whatever you return; it does not open a socket for you.

**Parameters** (`request`) - V1 shape:

| Field | Type | Description |
|---|---|---|
| `host_interface` | string | Interface to bind to (empty string means the loopback adapter). |
| `port` | number | Port to bind to (`0` means let the OS pick a random port). |
| `tls` | boolean | Whether TLS should be used, if your transport supports it. |
| `pact` | string | The whole Pact document as a JSON string. |
| `test_context` | table or nil | Context data from the test framework. |

**Parameters** (`request`) - V2 shape (used instead of the above when your manifest's `pluginInterfaceVersion` is `2`):

| Field | Type | Description |
|---|---|---|
| `host_interface` | string | Same as V1. |
| `port` | number | Same as V1. |
| `tls` | boolean | Same as V1. |
| `interactions` | array of tables | One [`InteractionContents`](#interactioncontents-table-v2-transport-only) per interaction, replacing the whole-Pact-JSON `pact` field. |
| `test_context` | table or nil | Same as V1. |

**Return value**: exactly one of:

- `{ error = "..." }` - the mock server failed to start.
- `{ details = { key = "...", port = ..., address = "..." } }`:

  | Field | Type | Description |
  |---|---|---|
  | `key` | string | An ID **you choose** to identify this running server - passed back to you in `shutdown_mock_server`/`get_mock_server_results`. |
  | `port` | number | The port the server ended up bound to. |
  | `address` | string | The address the server is reachable at (e.g. `"127.0.0.1:12345"` or a full URL, whatever's meaningful for your transport). |

---

### `shutdown_mock_server(server_key) -> table` and `get_mock_server_results(server_key) -> table`

Called to stop a running mock server (returning its final results) or to poll its results while it's still running,
respectively.

**Parameters**

| Name | Type | Description |
|---|---|---|
| `server_key` | string | The `key` you returned from `start_mock_server`'s `details` table. |

**Return value**: both functions return the same shape.

| Field | Type | Required | Description |
|---|---|---|---|
| `ok` | boolean | No (defaults to `true`) | Whether every configured interaction was matched with no errors. |
| `results` | array of tables | No | One entry per problem observed. Empty/absent means no problems. Each entry: |

Each entry in `results`:

| Field | Type | Required | Description |
|---|---|---|---|
| `path` | string | No | The path/identifier of the request this result is about. |
| `error` | string | No | A top-level error for this request, if any. |
| `mismatches` | string, table, or array | No | Same shape as a `match_contents` mismatch value (see [`match_contents`](#match_contentsrequest---table)) - a plain string, a [`ContentMismatch`](#contentmismatch-table) table, or an array of either. |

---

### `prepare_interaction_for_verification(request) -> table`

Called during provider verification, before the real request is made, to build the request data that will be sent.

**Parameters** (`request`) - V1 shape:

| Field | Type | Description |
|---|---|---|
| `pact` | string | The whole Pact document as a JSON string. |
| `interaction_key` | string | The key of the interaction being verified within `pact`. |
| `config` | table or nil | Data supplied by the user to verify the interaction. |

**Parameters** (`request`) - V2 shape (used instead of the above when your manifest's `pluginInterfaceVersion` is `2`):

| Field | Type | Description |
|---|---|---|
| `interaction_contents` | table or nil | An [`InteractionContents`](#interactioncontents-table-v2-transport-only), replacing `pact`/`interaction_key`. |
| `config` | table or nil | Same as V1. |
| `test_context` | table or nil | Context data from the test framework (V2 only - V1 has no equivalent field here). |

**Return value**: exactly one of:

- `{ error = "..." }` - preparation failed.
- `{ interaction_data = { body = <body table>, metadata = <metadata table> } }`:

  | Field | Type | Description |
  |---|---|---|
  | `body` | table or nil | A [`Body`](#body-table) - the request to be sent. |
  | `metadata` | table or nil | See [Metadata table](#metadata-table). |

---

### `verify_interaction(request) -> table`

Called to actually make the request against the real provider and compare the response. This is the one function
where your script is expected to perform real outbound network I/O.

**Parameters** (`request`) - V1 shape:

| Field | Type | Description |
|---|---|---|
| `interaction_data` | table or nil | Shaped like `prepare_interaction_for_verification`'s `interaction_data` return value: `{ body, metadata }`. |
| `config` | table or nil | Data supplied by the user to verify the interaction. |
| `pact` | string | The whole Pact document as a JSON string. |
| `interaction_key` | string | The key of the interaction being verified within `pact`. |

**Parameters** (`request`) - V2 shape (used instead of the above when your manifest's `pluginInterfaceVersion` is `2`):

| Field | Type | Description |
|---|---|---|
| `interaction_data` | table or nil | Same as V1. |
| `config` | table or nil | Same as V1. |
| `interaction_contents` | table or nil | An [`InteractionContents`](#interactioncontents-table-v2-transport-only), replacing `pact`/`interaction_key`. |
| `test_context` | table or nil | Context data from the test framework (V2 only). |

**Return value**: exactly one of:

- `{ error = "..." }` - the verification call itself failed (e.g. couldn't reach the provider at all).
- `{ result = { success, response_data, mismatches, output } }`:

  | Field | Type | Required | Description |
  |---|---|---|---|
  | `success` | boolean | No (defaults to `false`) | Whether the response matched with no mismatches. |
  | `response_data` | table or nil | No | Shaped like `interaction_data` above (`{ body, metadata }`) - the actual response received. |
  | `mismatches` | array | No | Each entry is either a plain string (a verification-level error) or a [`ContentMismatch`](#contentmismatch-table) table (a content mismatch). |
  | `output` | array of strings | No | Lines of human-readable output shown to the user (e.g. `"POST /foo"`, `"Received HTTP 200"`). |

---

## Common table shapes

### `Body` table

Used for request/response/message contents throughout (`configure_interaction`, `match_contents`,
`generate_content`, `interaction_data.body`, mock-server `details`, etc).

| Field | Type | Required | Description |
|---|---|---|---|
| `contents` | string or nil | No | The raw body bytes, as a Lua string. `nil` for an absent body. |
| `content_type` | string | Yes | MIME type, e.g. `"application/json"`. |
| `content_type_hint` | string | No | One of `"DEFAULT"`, `"TEXT"`, `"BINARY"`. Use `"BINARY"` for content whose body isn't actually parseable as its stated content type suggests (e.g. a compact JWT under `application/jwt+json` isn't JSON). Defaults to `"DEFAULT"` if absent. |

### `MatchingRules` table

The `rules` field of a `match_contents` request: a table keyed by matching-rule expression path (e.g. `"$"`,
`"$.body.field"`), where each value is an array of rule tables:

| Field | Type | Required | Description |
|---|---|---|---|
| `type` | string | Yes | The matching rule type (e.g. `"regex"`, `"type"`). |
| `values` | table or nil | No | Rule-specific configuration data. |

### `Generator` table

Each value in the `generators` table passed to `generate_content`:

| Field | Type | Required | Description |
|---|---|---|---|
| `type` | string | Yes | The generator type (e.g. `"RandomInt"`, `"Uuid"`). |
| `values` | table or nil | No | Generator-specific configuration data. |

### `PluginConfiguration` table

Used for `plugin_config`/`plugin_configuration` fields throughout:

| Field | Type | Required | Description |
|---|---|---|---|
| `interaction_configuration` | table or nil | No | Data scoped to a single interaction. |
| `pact_configuration` | table or nil | No | Data scoped to the whole Pact file (shared across interactions). |

### `ContentMismatch` table

Used wherever a mismatch is reported as a table rather than a plain string (`match_contents`, mock-server results,
`verify_interaction` mismatches):

| Field | Type | Required | Description |
|---|---|---|---|
| `mismatch` | string | Yes | Human-readable description of the mismatch. |
| `path` | string | No | Overrides the path this mismatch is reported under (otherwise the surrounding table's key/context path is used). |
| `expected` | string, number, or boolean | No | The expected value. Any scalar is accepted and stringified - it doesn't need to be a Lua string. |
| `actual` | string, number, or boolean | No | The actual value. Same as `expected`. |
| `diff` | string | No | A diff string to display, if you have one. |
| `mismatch_type` | string | No | A short type/category tag for the mismatch. |

### `InteractionContents` table (V2 transport only)

Structured per-interaction data sent to a V2-interface transport plugin in place of a whole Pact-as-JSON document,
used in `start_mock_server`'s `interactions` array and as `prepare_interaction_for_verification`/
`verify_interaction`'s `interaction_contents` field:

| Field | Type | Required | Description |
|---|---|---|---|
| `interaction_type` | string | Yes | The V4 interaction type, e.g. `"Synchronous/HTTP"`, `"Synchronous/Messages"`. |
| `consumer` | string | Yes | Consumer name (for result reporting/log correlation). |
| `provider` | string | Yes | Provider name (for result reporting/log correlation). |
| `plugin_configuration` | table or nil | No | A [`PluginConfiguration`](#pluginconfiguration-table). |

### Metadata table

Used for `interaction_data.metadata` (in `prepare_interaction_for_verification`'s and `verify_interaction`'s
request/response `interaction_data`). A table keyed by metadata name, where each value is one of:

- a plain Lua value (string, number, boolean, or nested table) - a non-binary (JSON-like) metadata value; or
- `{ binary = "..." }` - a wrapper table whose `binary` field is a Lua string of raw bytes, for a binary metadata
  value (e.g. raw header bytes). This is the only way to distinguish a binary value from a non-binary one, since
  Lua doesn't otherwise distinguish a "string" from a "byte string".

```lua
metadata = {
  path = "/foo",              -- non-binary (string)
  retries = 3,                 -- non-binary (number)
  signature = { binary = "\x01\x02\x03" }  -- binary
}
```

## Host functions available to your script

The driver registers these as Lua globals before loading your script:

| Function | Description |
|---|---|
| `logger(message)` | Writes a diagnostic message to the per-instance log file. |
| `rsa_sign(data, privateKeyPem)` | Signs `data` with an RSA private key (PKCS#1 PEM, RS512), returns a URL-safe base64 (no padding) signature string. |
| `rsa_public_key(privateKeyPem)` | Derives the PKCS#1 PEM public key from an RSA private key PEM. |
| `rsa_validate(tokenParts, algorithm, publicKeyPem)` | Verifies a 3-part token (`{header, payload, signature}`) against an RSA public key PEM. Only `"RS512"` is supported for `algorithm`. Returns a boolean. |
| `b64_decode_no_pad(data)` | Decodes URL-safe base64 (with or without padding), returns the raw bytes as a Lua string. |

These exist specifically to support the JWT reference plugin. If your plugin needs different cryptographic,
encoding, or networking primitives, either implement them in pure Lua or pull in a
[pure-Lua LuaRocks package](writing-plugin-guide.md#luarocks-support) that provides them - packages with compiled C
extensions are not supported.

Lua's built-in `print(...)` is also redirected into the same per-instance log file as `logger(...)`, rather than
the driver's own real stdout - see [Output and logging](writing-plugin-guide.md#output-and-logging).
