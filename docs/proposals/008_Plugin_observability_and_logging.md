# Plugin observability and logging

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

## Evidence from trace logs

Analysis of captured trace logs from both the Rust and JVM drivers (CSV plugin, 3-test run) reveals four concrete
problems.

### Problem 1 — Transport chatter dominates at TRACE level

The Rust driver trace log for a single ~1-second test run is 1.7 MB. The overwhelming majority of this volume comes
from `h2`, `tonic`, `hyper_util`, and `tracing::span` — HTTP/2 frame-level machinery that is completely unrelated to
plugin behaviour:

```
[TRACE h2::codec::framed_write] FramedWrite::buffer; frame=Settings { flags: (0x0), ...
[TRACE tonic::transport::channel::service::reconnect] poll_ready; connecting
[TRACE tracing::span::active] -> FramedWrite::buffer;
[TRACE h2::proto::streams::flow_control] inc_window; sz=65535; old=0; new=65535
```

This noise appears **twice** — once from the driver itself, and again forwarded from the plugin's stderr (the CSV
plugin is also Rust and uses the same tonic stack). A developer enabling TRACE to debug a plugin matching problem
immediately drowns in TCP window size updates and HTTP/2 handshake spans.

### Problem 2 — Plugin stderr is a log-within-a-log

Plugin stderr is forwarded line-by-line as driver DEBUG lines:

```
[DEBUG pact_plugin_driver::child_process] Plugin(csv, 555931, STDERR) || [2026-06-12T00:48:44Z DEBUG pact_csv_plugin] Received configure_contents request for 'text/csv'
```

The plugin's own meaningful log messages (matching rule parsing, column definitions, generator assignments) are
embedded inside driver log records. This structure makes it impossible to use standard log filtering: you cannot grep
for `pact_csv_plugin` without the driver wrapper around every line, and you cannot easily separate plugin logs from
driver logs.

### Problem 3 — No test correlation anywhere in the logs

Nothing in either the Rust or JVM trace logs identifies which test a log line belongs to. Two separate
`ConfigureContents` calls for two different interactions appear at the same timestamp with no distinguishing context.
When tests run in parallel, logs from concurrent interactions are completely interleaved with no way to reconstruct
the per-test sequence.

The `testContext` field already exists in `GenerateContentRequest`, `StartMockServerRequest`,
`VerificationPreparationRequest`, and `VerifyInteractionRequest` — but it is **absent** from
`ConfigureInteractionRequest` and `CompareContentsRequest`, which are the two primary consumer-side plugin calls.
This is where test correlation is most needed.

### Problem 4 — Plugin instance ID is missing from the handshake

`InitPluginRequest` carries no plugin instance identifier. If two instances of the same plugin are running
simultaneously (e.g. two test threads both loaded the protobuf plugin), their log output cannot be separated: both
use the same logger name and there is no shared ID that the driver assigned at startup to tie log lines back to the
specific process.

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

### Correlation IDs

Two correlation IDs are needed to make log output traceable across a driver and its plugins:

- **Plugin instance ID** — assigned by the driver when the plugin process is started and passed to the plugin in
  `InitPlugin`. All log output from that plugin instance carries this ID, making it possible to separate logs from
  multiple concurrently running instances of the same plugin.
- **Test run ID** — supplied by the test framework and passed into each plugin call (via `testContext` or an
  equivalent field). This allows all log output related to a single test or verification run to be correlated across
  the driver and any plugins it called, even when multiple tests are running in parallel.

Both IDs must be included in every structured log record emitted by the plugin. For gRPC plugins the driver passes
them as fields in the relevant request messages; for WASM plugins they are passed as arguments to the host log
function.

### Protocol changes required (V2 only)

V1 is a frozen interface. Changing V1 messages would break existing plugins that implement them, and any plugin that
needs these new fields should migrate to V2 rather than receiving a partial backport. All changes below apply
exclusively to `plugin_v2.proto`.

#### Add `pluginInstanceId` to `InitPluginRequest`

The driver generates a UUID when it starts each plugin process and passes it here. The plugin stores this ID and
includes it in every log record it emits.

```proto
message InitPluginRequest {
  string implementation = 1;
  string version = 2;
  repeated string hostCapabilities = 3;
  string pluginInstanceId = 4;  // UUID assigned by the driver at process start
}
```

#### Add `testContext` to `ConfigureInteractionRequest` and `CompareContentsRequest`

Both of these messages are missing `testContext` in V2, yet they are the core consumer-test-side plugin calls where
test correlation is most needed.

```proto
message ConfigureInteractionRequest {
  string contentType = 1;
  google.protobuf.Struct contentsConfig = 2;
  google.protobuf.Struct testContext = 3;  // add
}

message CompareContentsRequest {
  Body expected = 1;
  Body actual = 2;
  bool allow_unexpected_keys = 3;
  map<string, MatchingRules> rules = 4;
  PluginConfiguration pluginConfiguration = 5;
  google.protobuf.Struct testContext = 6;  // add
}
```

#### Add a `Log` RPC (driver-side server)

Plugins need a way to forward structured log records back to the driver without relying on stderr capture. This is
the only mechanism that works uniformly for both gRPC and WASM plugins. The driver exposes this endpoint; plugins
call it.

```proto
message LogMessage {
  string pluginInstanceId = 1;
  string testRunId = 2;       // from testContext, if available
  string level = 3;           // TRACE, DEBUG, INFO, WARN, ERROR
  string message = 4;
  string target = 5;          // logger name / module path
  int64 timestampMs = 6;
}

// Driver-side service (called by plugins, implemented by the driver)
service PluginHost {
  rpc Log(LogMessage) returns (google.protobuf.Empty);
}
```

The driver address/port for this service is passed to the plugin at startup (either as an `InitPluginRequest` field
or as an environment variable set before the process is launched).

### Driver-side changes required

#### Two complementary log paths

Plugin log output flows through two distinct paths that serve different purposes and should both be preserved:

**Path 1 — `Log` RPC → user's logging framework**

When a plugin calls the `Log` RPC, the driver receives a structured `LogMessage` and forwards it into whatever
logging implementation the user's project is already using (SLF4J/Logback for JVM, `tracing`/`log` for Rust, etc.).
The record appears in normal test output at the appropriate level, with the same format and filtering rules as the
rest of the project's logs. This is the primary path for meaningful, human-readable plugin diagnostics during a test
run.

**Path 2 — stderr capture → per-instance log file**

The driver continues to capture plugin stderr unconditionally and writes it to a dedicated file:

```
<pact-output-dir>/logs/pact-plugin-<name>-<instanceId>.log
```

This file is the safety net for cases the `Log` RPC cannot cover:

- **Crash diagnostics** — panic messages and stack traces go to stderr; the `Log` RPC will not be called during a
  crash. Without this file, crash output is lost.
- **Pre-connection failures** — if the plugin fails before it can connect back to the driver's `Log` endpoint (bad
  config, missing dependency, port conflict), stderr is the only channel available.
- **V1 plugin compatibility** — V1 plugins have no `Log` RPC; stderr-to-file is their only log path.
- **Unstructured output from dependencies** — plugin libraries may log directly to stderr without going through the
  plugin's log facade.

The driver log itself records only lifecycle events at INFO/DEBUG (plugin started with PID, port, and instance ID;
plugin stopped). It no longer echoes plugin stderr inline, eliminating the log-within-a-log anti-pattern.

#### Suppress third-party transport trace output

The `h2`, `tonic`, `hyper_util`, and `tracing::span` crates should never appear in plugin diagnostic output
regardless of the configured log level. In the Rust driver, enforce a maximum effective level of `WARN` for these
targets using a log filter directive applied at startup. Plugin authors and users diagnosing plugin problems have no
use for HTTP/2 frame encoding traces; those belong in a separate transport debug mode.

In the JVM driver, the equivalent Netty and gRPC-Netty loggers should be capped at WARN for the same reason.

#### Populate `testContext` with a test run ID

The test framework integration layer (JUnit 5 extension, Rust `pact_consumer` crate) should generate a UUID per test
and place it in `testContext` under the key `"testRunId"`. Drivers should propagate this value from `testContext`
into the `testRunId` field of any `LogMessage` they forward. This is the minimum needed to correlate driver and
plugin logs for a single test without requiring changes to the core pact model.

#### Driver-level log sink (Rust driver)

The Rust driver has no plugin manager instance — all state is held in static globals (`PLUGIN_REGISTER`,
`PLUGIN_MANIFEST_REGISTER`). There is therefore no object on which to register a log handler. Instead, the driver
should maintain a static, replaceable log sink, following the same pattern as the `log` crate's `set_logger`.

Define a `PluginLogSink` trait in the driver:

```rust
pub trait PluginLogSink: Send + Sync {
  fn log(&self, entry: &PluginLogEntry);
}
```

A global static holds the active sink (defaulting to an implementation that forwards entries into the `tracing`
subsystem). A registration function lets the embedding layer swap it out once at startup:

```rust
pub fn set_plugin_log_sink(sink: Box<dyn PluginLogSink>);
```

The driver calls `sink.log(entry)` in two places:
- In `child_process.rs`, when a line is read from plugin stderr (replacing the current `debug!()` call).
- In the `PluginHost` gRPC service handler, when a `LogMessage` arrives via the `Log` RPC (once that is implemented).

The driver has no knowledge of pact_ffi, C callbacks, or buffering — those are concerns of whatever sink is
registered. The default sink routes entries through `tracing` so the driver works correctly on its own without any
sink being registered.

#### FFI consumer log forwarding (`pact_ffi`)

Several Pact implementations (JavaScript, Go, Python, .NET) use the `pact_ffi` library rather than a native driver.
These consumers cannot tap into the Rust `tracing` subscriber directly, so pact_ffi registers its own
`PluginLogSink` implementation against the driver's static sink at initialisation time.

Two mechanisms are needed, and both should be provided:

**Callback (real-time forwarding)**

The FFI consumer registers a function pointer once at startup. The driver calls it for each `LogMessage` received
via the `Log` RPC, allowing the consumer to route entries into their native logging framework as they arrive. This
follows the same pattern as the existing pact_ffi log initialisation functions rather than introducing a separate
mechanism.

```c
typedef void (*PluginLogCallback)(const char *plugin_instance_id,
                                  const char *test_run_id,
                                  const char *level,
                                  const char *target,
                                  const char *message);

void pactffi_register_plugin_log_callback(PluginLogCallback callback);
```

The callback will be invoked from the tokio runtime thread that handles the gRPC `Log` RPC, so consumers must ensure
their callback implementation is thread-safe.

**Capture and retrieve (post-test access)**

The driver always buffers log entries per plugin instance in memory, regardless of whether a callback is registered.
After a test completes the consumer can retrieve the buffered entries:

```c
// Returns a newline-delimited JSON string of LogMessage records for the given instance.
// Caller is responsible for freeing the returned string.
const char *pactffi_get_plugin_logs(const char *plugin_instance_id);
```

Buffering is unconditional because a callback alone is insufficient: if the test process crashes or the callback
throws, entries recorded before the failure would otherwise be lost. The buffer is cleared when the plugin instance
is shut down.

Consumers that want live forwarding register a callback; consumers that only need post-test diagnostics call
`pactffi_get_plugin_logs`. Both paths can be used together.

## Non-goals for this proposal

- Defining the plugin callback protocol.
- Redesigning verification payloads or field-level matcher APIs.
- Replacing the existing plugin manifest format.

## WASM compatibility

WASM plugins do not have access to stdout or stderr by default. WASI provides optional I/O but it cannot be relied
upon as a universal mechanism. Any logging strategy that depends on capturing process output does not apply to WASM
plugins.

Structured log forwarding via the `Log` RPC (host function call for WASM) must therefore be the primary logging
mechanism, so that the same approach works for both gRPC and WASM plugins. For gRPC plugins this can be complemented
by file-based output, but that cannot be the only path.

Delivering this before the Phase 3 functionality proposals ensures that field-level matchers, generators, and
callbacks all have a consistent logging path from the start rather than retrofitting observability later.

## Resolved questions

- **Should drivers capture plugin logs to per-run files, stream them through structured logging, or both?**
  Per-instance files are the primary output for gRPC plugins. Structured log forwarding via `Log` RPC is the primary
  path for WASM and the preferred path for gRPC plugins that want structured output. Both mechanisms write to the
  same per-instance file on the driver side.

- **What minimum structured diagnostics should every plugin provide?**
  Every plugin must include `pluginInstanceId` and `testRunId` (when available) in every log record. Startup/shutdown
  events (process start, init request received, catalogue registered, shutdown) are mandatory at INFO level.

- **How should user-facing plugin output be separated from low-level debug logs?**
  Use the `level` field in `LogMessage`. Driver UIs and test reporters display INFO and above by default. TRACE/DEBUG
  records go to the per-instance log file only and are never shown in test output unless the user explicitly requests
  verbose plugin diagnostics.
