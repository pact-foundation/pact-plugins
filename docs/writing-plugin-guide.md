# Guide to writing a Pact plugin

Pact plugins are essentially gRPC servers that run as child processes to the main Pact process (whether in a consumer
test or during provider verification). They are designed to be stateless and respond to requests from the Pact framework
that is running the tests. They can be written in any language that has gRPC support, but ideally should
be written in a language that has minimal system dependencies.

Alternatively, a content-matcher/content-generator plugin can be written in **Lua** and embedded directly in the
driver's own process instead of run as a separate gRPC child process - see
[Writing plugins in Lua](#writing-plugins-in-lua) below.

**IMPORTANT NOTE:** Please keep the end users in mind when selecting a language to write a plugin in. If you use, say
Java, that means any user who uses your plugin needs to have a JDK installed on their machines and CI servers, as well
as anything that verifies a Pact file created using that plugin even if the provider is written in a different language.

There are two prototype example plugins, one for [CSV](../plugins/csv) and one for [Protobuf](../plugins/protobuf). 
You can find examples of consumer and provider tests using these plugins in the [examples](../examples) in this repository.
The CSV plugin is written in Rust and the Protobuf one in Kotlin. There is also a [JWT](../plugins/jwt) plugin written
in Lua - see [Writing plugins in Lua](#writing-plugins-in-lua).

## Plugin Interface

The first version of the plugin interface (version 1) supports extending Pact with [Content](content-matcher-design.md) types, along with using Matchers and Generators to support flexible matching, and for adding new [Transport protocols](protocol-plugin-design.md).

Content Matchers are for things like request/response bodies and message payloads and are based on specified MIME types, such as protobufs. 

Transports allow you to communicate these new content types over a different wire protocol, such as gRPC or Websockets.

Refer to for more details on the interface and gRPC methods that need to be implemented:

- [Content Matchers and Generators](content-matcher-design.md)
- [Transport protocols](protocol-plugin-design.md)

You can find the [proto file](../proto/plugin.proto) that defines the plugin interface in the proto directory. Your 
plugin will need to implement this interface.

## Plugin Interface Version 2

Version 2 of the plugin interface ([plugin_v2.proto](../proto/plugin_v2.proto)) extends v1 with capability
negotiation and structured log forwarding. A v2 plugin sets `pluginInterfaceVersion: 2` in its manifest.

### Capability negotiation

The `InitPluginRequest` in v2 adds two fields:

- `hostCapabilities` — list of capability strings advertised by the driver (e.g. `"content-matcher/csv"`). Your
  plugin can inspect these to decide which optional features to enable.
- `pluginInstanceId` — a UUID assigned by the driver to this specific instance of your plugin. Store this value
  and include it in every `LogMessage` you send (see below).

### Test context

Several request types (`CompareContentsRequest`, `ConfigureInteractionRequest`, etc.) carry a `testContext` struct.
This is freeform data from the test framework. The key field of interest is `testContext["testRunId"]`: a UUID
identifying the current test run, useful for correlating log messages back to a specific test.

### Logging

#### Per-instance log file

The driver captures everything your plugin writes to **stderr** and saves it to a per-instance log file. The
location depends on whether `PACT_OUTPUT_DIR` is set in the environment:

| Condition | Log file path |
|-----------|--------------|
| `PACT_OUTPUT_DIR` is set | `$PACT_OUTPUT_DIR/logs/pact-plugin-<name>-<instanceId>.log` |
| Default | `~/.pact/plugins/logs/pact-plugin-<name>-<instanceId>.log` |

This file receives the complete log output of your plugin at whatever level the framework configures, including
TRACE. You do not need to do anything special — just write to stderr as normal.

#### Forwarding logs to the driver via Log RPC

If the `PACT_PLUGIN_HOST` environment variable is set when your plugin starts, the driver is offering a
`PluginHost` gRPC endpoint. You should connect to it and forward **DEBUG-level and above** log records via the
`Log` RPC, with two exceptions that must be excluded to keep test output clean:

- **TRACE records** — too verbose; captured in the log file only.
- **Transport-layer targets** — records whose logger name/target starts with `h2::`, `tower::`, `tonic::`,
  `hyper_util::`, or `hyper::` are gRPC transport internals, not plugin application output. Forward them to
  stderr as normal but do not send them via the Log RPC.

The `PACT_PLUGIN_INSTANCE_ID` environment variable is also set by the driver before your plugin starts. Read it
at startup and use it as `pluginInstanceId` in every `LogMessage`. Do not wait for `InitPluginRequest` to arrive
before setting this value — the driver expects log messages with the correct instance ID from the moment your
plugin begins connecting.

A minimal forwarding implementation:

1. Read `PACT_PLUGIN_INSTANCE_ID` from the environment at startup and store it.
2. If `PACT_PLUGIN_HOST` is set, open a gRPC connection to that address (plain-text, no TLS).
3. For each log record at DEBUG or above, call `PluginHost.Log` with a `LogMessage` containing:
   - `pluginInstanceId` — the value from `PACT_PLUGIN_INSTANCE_ID`
   - `testRunId` — extracted from `testContext["testRunId"]` if a test context is active, otherwise empty
   - `level` — one of `DEBUG`, `INFO`, `WARN`, `ERROR`
   - `message` — the log message text
   - `target` — the logger name or module path (e.g. `"my_plugin::matching"`)
   - `timestampMs` — Unix epoch milliseconds
4. Always also write the record to stderr so it appears in the log file regardless of whether the Log RPC
   connection is available.

See the [CSV plugin](../plugins/csv/src/main.rs) for a complete Rust reference implementation.

When the plugin starts up, it needs to write a small JSON message to standard output that contains the port the plugin
is running on and an optional server key. The port should be one assigned by the operating system so there are no clashes
with other servers. The server key is reserved for use as a bearer token to restrict access to the
plugin from the Pact framework that started it. Ideally, the plugin gRPC server should bind to the loopback interface (127.0.0.1),
but this may not always be possible so if the plugin binds to all interfaces, the server key would provide a security
mechanism to not allow just any process to invoke the plugin methods.

You can see the prototype plugins doing this if you run their executable:

```commandline
$ ~/.pact/plugins/csv-0.0.0/pact-plugin-csv
{"port":35517, "serverKey":"56f7eb63-073b-429c-bff4-6ad336163067"}
```

Refer to the [Plugin drivers](plugin-driver-design.md) for more details.

## Writing plugins in Lua

As an alternative to a compiled gRPC server, a content-matcher/content-generator plugin can be written in **Lua**
and run embedded directly in the driver's own process, instead of as a separate child process. This trades away
language-agnosticism (only Lua is supported this way) for a much simpler authoring experience: no gRPC boilerplate,
no protobuf code generation, and no separate executable to build and distribute per OS/architecture - just `.lua`
files.

Both drivers embed a real Lua 5.4 interpreter, so a script behaves identically on either one:

- The Rust driver embeds Lua via [`mlua`](https://crates.io/crates/mlua) (feature `lua`, enabled by default).
- The JVM driver embeds Lua via [`party.iroiro/luajava`](https://github.com/gudzpoz/luajava), a JNI binding to the
  real Lua 5.4 C library, behind a small `LuaEngine` abstraction
  (`drivers/jvm/core/.../lua/LuaEngine.kt`) so the underlying binding could be swapped out later.

See the [JWT plugin](../plugins/jwt) for a complete, working reference implementation, and
[its examples](../examples/jwt) for consumer/provider tests exercising it on both drivers.

### Scope

Only content matching and generation is supported: `compareContents`, `configureInteraction`, and `generateContent`.
Mock-server hosting and provider-verification RPCs (`verifyInteraction`/`prepareInteractionForVerification`) are
only ever invoked for `TRANSPORT`-registered plugins, never for `CONTENT_MATCHER`/`CONTENT_GENERATOR` ones (see
[Content Matchers and Generators](content-matcher-design.md)), so there's no reason to support them for a Lua
plugin - a Lua plugin gets full provider-verification support for free through `compareContents`/`generateContents`
alone, since the core verifier makes the real request to the provider and then reuses ordinary content matching to
compare the actual response against the expected one.

### Manifest

Set `executableType` to `"lua"` and `entryPoint` to the relative path of your entry point script, for example:

```json
{
  "manifestVersion": 1,
  "pluginInterfaceVersion": 1,
  "name": "jwt",
  "version": "0.0.0",
  "executableType": "lua",
  "entryPoint": "plugin.lua"
}
```

There's no `entryPoints`-per-OS variant, no `args`, and no executable to gzip/tar for a release - a Lua plugin is
just the script files plus this manifest, installed the same way as any other plugin (see
[Installing your plugin](#installing-your-plugin) below): copied into
`~/.pact/plugins/<name>-<version>/` (or `$PACT_PLUGIN_DIR/<name>-<version>/`).

### Entry point contract

Your entry point script must define these global functions. Request/response "tables" below use plain Lua
tables with string keys, mapping directly to the fields of the corresponding gRPC message (see the
[proto file](../proto/plugin.proto)) - the driver converts between the two automatically.

- **`init(implementation, version) -> table`** - called once, right after your script is loaded. Must return an
  array of catalogue entries, each shaped as:
  ```lua
  { entryType = "CONTENT_MATCHER", key = "jwt", values = { ["content-types"] = "application/jwt" } }
  ```
  `entryType` is `"CONTENT_MATCHER"` or `"CONTENT_GENERATOR"`; `values["content-types"]` is a semicolon-separated
  list of MIME types your plugin handles, matched as a regex against the actual content type (anchored - the
  *whole* type must match, not just part of it), so any regex metacharacter in a content type (most commonly `+`,
  as in a `+json`/`+xml` structured syntax suffix) needs to be escaped for a literal match - see `plugin.lua` in
  the JWT plugin for a worked example.
- **`configure_interaction(content_type, config) -> table`** - called once per interaction part (request or
  response) when a consumer test configures your content type. `config` is the table of data the user specified in
  their test. Must return:
  ```lua
  {
    interactions = {
      { contents = { contents = "...", content_type = "...", content_type_hint = "TEXT" }, part_name = "" }
    },
    plugin_config = { interaction_configuration = { ... }, pact_configuration = { ... } }
  }
  ```
  `content_type_hint` is one of `"DEFAULT"`, `"TEXT"`, or `"BINARY"` - use `"BINARY"` for any content whose body
  isn't actually parseable as its stated content type suggests (for example, a compact JWT under
  `application/jwt+json` isn't JSON; see the note in `plugin.lua`). `plugin_config` is arbitrary data your plugin
  needs persisted into the Pact file (e.g. a public key derived during configuration, so verification can validate
  a signature without ever needing the private key) - it's handed back to you as
  `plugin_configuration` in later `match_contents` calls for the same interaction.
- **`match_contents(request) -> table`** - called to compare actual content against expected content. `request`
  has `expected`/`actual` (each a body table like above, or `nil`), `allow_unexpected_keys` (boolean), `rules`
  (a table keyed by matching-rule expression, each value an array of `{ type = "...", values = {...} }`), and
  `plugin_configuration` (whatever you returned from `configure_interaction`). Return one of:
  - `{ error = "..." }` - a hard error; verification is marked failed.
  - `{ ["type-mismatch"] = { expected = "...", actual = "..." } }` - the content types themselves didn't match.
  - `{ mismatches = { ["$"] = {...}, ["some.path"] = {...} } }` - a table keyed by matching-rule-expression path;
    each value is either a plain string, a table `{ mismatch = "...", path = "...", expected = ..., actual = ...,
    diff = "...", mismatch_type = "..." }`, or an array of either. An empty (or absent) `mismatches` table means
    the content matched.
- **`generate_content(contents, generators, test_mode)` (optional)** - called to generate contents using any
  defined generators. `test_mode` is `"Consumer"` or `"Provider"`. If you don't define this function, the driver
  passes the original `contents` through unchanged - reasonable for content (like a JWT) that has nothing to
  generate field-by-field.
- **`update_catalogue(catalogue)` (optional)** - called whenever another plugin loads and the combined catalogue
  changes. If you don't define this function, it's a no-op.

### Host functions available to your script

The driver registers a few host (native) functions as Lua globals before loading your script:

- **`logger(message)`** - writes a diagnostic message (see [Output and logging](#output-and-logging) below).
- **`rsa_sign(data, privateKeyPem)`**, **`rsa_public_key(privateKeyPem)`**, **`rsa_validate(tokenParts, algorithm,
  publicKeyPem)`**, **`b64_decode_no_pad(data)`** - RSA (RS512/PKCS#1 PEM) signing/verification and base64
  decoding primitives, since Lua has no built-in crypto support. These exist specifically to support the JWT
  reference plugin; if your plugin needs different cryptographic or encoding primitives, either implement them in
  pure Lua or pull in a [LuaRocks package](#luarocks-support) that provides them.

### LuaRocks support

Pure-Lua packages installed via [LuaRocks](https://luarocks.org/) are available to `require` in your script,
without needing to vendor every third-party library you depend on. Both drivers add the standard LuaRocks
per-Lua-version tree layout to `package.path`:

```
<rocks_dir>/share/lua/5.4/?.lua
<rocks_dir>/share/lua/5.4/?/init.lua
```

`<rocks_dir>` defaults to `~/.luarocks` (LuaRocks' standard per-user tree). Your plugin can override it with a
`luaRocksDir` key in the manifest's `pluginConfig`:

```json
{
  "executableType": "lua",
  "entryPoint": "plugin.lua",
  "pluginConfig": {
    "luaRocksDir": "/custom/path/to/rocks/tree"
  }
}
```

If the resulting `share/lua/5.4` directory doesn't exist (default or configured), it's silently skipped rather
than erroring, since not every Lua plugin needs rocks. Only pure-Lua packages are supported - packages with
compiled C extensions (under a rocks tree's `lib/lua`) are not, since those would need to be compiled
per-platform, which defeats much of the point of writing a plugin in Lua in the first place.

### Output and logging

Lua plugins don't have their own OS-level stdout/stderr the way a gRPC child process does - they run embedded in
the driver's own process. Calling Lua's built-in `print(...)` (or the `logger(message)` host function above) is
redirected into exactly the same per-instance log file a gRPC plugin's stderr is captured to (see
[Per-instance log file](#per-instance-log-file) above): `<pact-dir>/logs/pact-plugin-<name>-<instanceId>.log`. You
don't need to do anything special - just call `print(...)` or `logger(...)` as normal, and check that file if
something isn't behaving as expected.

## Plugin manifest

Each plugin needs to have a manifest file named `pact-plugin.json` in JSON format that describes how the plugin should 
be loaded and any dependencies it requires. The format of the manifest is documented in [Plugin drivers](plugin-driver-design.md). 
This file needs to be installed alongside your plugin executable files. Refer to the [CSV](../plugins/csv/pact-plugin.json) 
and [Protobuf](../plugins/protobuf/pact-plugin.json) manifest files for examples.

The important attribute in the manifest is the `entryPoint`. This is the executable that starts your plugin. The Protobuf
example also has an additional entry for Windows, because it uses batch files to start.

## Installing your plugin

By default, each plugin is installed (along with its manifest) in a directory named `<plugin name>-<version>` in 
the `.pact/plugins` directory in the users home directory. This default can be changed with the `PACT_PLUGIN_DIR`
environment file. `<plugin-name>` is the name of the plugin (corresponding to the name in the manifest) and `<version>`
if the version of the plugin. This way users can have different versions of your plugin installed.

Looking at the `.pact/plugins` on my machine we can see I have the two prototype plugins installed:

```commandline
$ ls -l ~/.pact/plugins/
total 8
drwxrwxr-x 2 ronald ronald 4096 Oct 18 14:09 csv-0.0.0
drwxrwxr-x 6 ronald ronald 4096 Oct 13 15:21 protobuf-0.0.0

$ ls -l ~/.pact/plugins/csv-0.0.0/
total 12376
-rwxrwxr-x 1 ronald ronald 12667032 Oct  6 12:04 pact-plugin-csv
-rw-rw-r-- 1 ronald ronald      237 Oct 18 14:09 pact-plugin.json

$ ls -l ~/.pact/plugins/protobuf-0.0.0/
total 28
drwxr-xr-x 2 ronald ronald  4096 Aug 27 12:40 bin
drwxr-xr-x 2 ronald ronald 12288 Oct 13 12:23 lib
-rw-rw-r-- 1 ronald ronald   352 Oct 11 11:07 pact-plugin.json
drwxrwxr-x 2 ronald ronald  4096 Oct 18 13:33 tmp
```

### Installing using the [pact-plugin-cli](https://github.com/pact-foundation/pact-plugins/tree/main/cli)

The `pact-plugin` command can be used to manage plugins. To be able to install your plugin, the CLI tool requires:

* Plugin is released via GitHub releases with attached installation files.
* The plugin manifest file must be attached to the release and have the correct name and version.
* For single executable plugins, the executable attached to the release must be gzipped and named in the form `pact-${name}-plugin-${os}-${arch}(.exe?).gz`
  * `name` is the name from the plugin manifest file
  * `os` is the operating system (linux, windows, osx)
  * `arch` is the system architecture (x86_64, aarch64 for Apple M1. See https://doc.rust-lang.org/stable/std/env/consts/constant.ARCH.html)
  * Windows executables require `.exe` extension in the filename. Leave this out for Unix and OSX.
* For bundled plugins (like with Node.js or Java), you can use a Zip or Tar.gz file. The file must be named `pact-${name}-plugin.zip` or `pact-${name}-plugin-${os}-${arch}.zip` if you have OS/arch specific bundles. If using tarballs, `.tar.gz` or `.tgz` is supported.

If you provide SHA256 files (with the same name but with `.sha256` appended), the installation command will check the downloaded
artifact against the digest checksum in that file. For example, the Protobuf plugin executable for Linux is named 
`pact-protobuf-plugin-linux-x86_64.gz` and the digest `pact-protobuf-plugin-linux-x86_64.gz.sha256`.

#### Adding plugins to the `pact-plugin-cli` index

The `pact-plugin` has a built-in index of known plugins which can be installed by name. For example, to install the
Protobuf plugin, run `pact-plugin install protobuf` and it will know how to download that plugin from the index.

You can add entries to the index using the `pact-plugin repository` commands. The index files are checked in to
https://github.com/pact-foundation/pact-plugins/tree/main/repository. So the steps to add a new plugin or plugin 
version are (using the [AVRO plugin](https://github.com/austek/pact-avro-plugin) as an example):

1. Fork and clone the https://github.com/pact-foundation/pact-plugins repo.
2. You can list the current index and also validate it with:
```console
❯ pact-plugin repository list repository/repository.index
┌──────────┬──────────┬────────────────┬──────────┐
│ Key      ┆ Name     ┆ Latest Version ┆ Versions │
╞══════════╪══════════╪════════════════╪══════════╡
│ csv      ┆ csv      ┆ 0.0.3          ┆ 4        │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
│ protobuf ┆ protobuf ┆ 0.3.0          ┆ 29       │
└──────────┴──────────┴────────────────┴──────────┘

❯ pact-plugin repository validate repository/repository.index
'/home/ronald/Development/Projects/Pact/pact-plugins/repository/repository.index' OK

┌────────────────┬──────────────────────────────────────────────────────────────────┬─────────────────────────────────────────────┐
│ Key            ┆ Value                                                            ┆                                             │
╞════════════════╪══════════════════════════════════════════════════════════════════╪═════════════════════════════════════════════╡
│ Format Version ┆ 0                                                                ┆                                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Index Version  ┆ 5                                                                ┆                                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Last Modified  ┆ 2023-03-10 05:36:45.725083896 UTC                                ┆ Local: 2023-03-10 16:36:45.725083896 +11:00 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Plugin Entries ┆ 2                                                                ┆                                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ Total Versions ┆ 33                                                               ┆                                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ SHA            ┆ d41450c9849112b08a8633c27893ac2ec0e9fe958e0861e570083da7b307ad56 ┆                                             │
└────────────────┴──────────────────────────────────────────────────────────────────┴─────────────────────────────────────────────┘
```
3. Add a new entry for the plugin. You can also get it to scan the GitHub project to add all versions.
```console
❯ pact-plugin repository add-plugin-version git-hub repository/repository.index https://github.com/austek/pact-avro-plugin/releases/tag/v0.0.3
Added plugin version avro/0.0.3 to repository file '/home/ronald/Development/Projects/Pact/pact-plugins/repository/repository.index'

❯ pact-plugin repository list repository/repository.index
┌──────────┬──────────┬────────────────┬──────────┐
│ Key      ┆ Name     ┆ Latest Version ┆ Versions │
╞══════════╪══════════╪════════════════╪══════════╡
│ avro     ┆ avro     ┆ 0.0.3          ┆ 1        │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
│ csv      ┆ csv      ┆ 0.0.3          ┆ 4        │
├╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌┤
│ protobuf ┆ protobuf ┆ 0.3.0          ┆ 29       │
└──────────┴──────────┴────────────────┴──────────┘
```
4. Then commit the changed files and create a PR.

### If your plugin needs to use disk storage

By default, the plugins should be stateless. They will receive all the required data from Pact framework running the test.
However, if they need to use disk space, they should only write to files within the plugins installed directory. Some
versions of Unix or docker containers may not allow writing to the `/tmp` directory, and you won't know how the Pact tests
are going to be run.

If you look at the Protobuf directory above, you can see a `tmp` directory. This is where the proto file is written to be
passed to the protoc compiler and where the resulting proto descriptor is written. You should also clean up any files
written within the plugin directory.

When the plugin process is started, the current working directory will be set to the plugin's installed directory, so you
can use relative paths to load or write any files. The Protobuf plugin uses the relative path `./tmp` for the proto files.

## Plugin lifecycle

The plugin process will be started when the Pact framework detects that it is needed. This will be when a consumer test
runs that specifies that the plugin must be loaded or when a Pact file that needs the plugin is loaded to be verified. The
plugin driver library will control this. Ideally the plugin process will be kept running for as long as needed, but it may
also be started and stopped for each test. So don't rely on it being a long running process.

