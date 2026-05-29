# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository structure

This is **not** a single top-level workspace. Run all build/test commands from within the relevant component directory.

| Directory | Description |
|-----------|-------------|
| `proto/` | Shared gRPC contract files (`plugin.proto` v1, `plugin_v2.proto` v2) |
| `drivers/rust/` | Rust plugin driver (3 crates: `driver/`, `driver_ffi/`, `driver_pact_tests/`) |
| `drivers/jvm/` | JVM (Kotlin) plugin driver |
| `cli/` | Rust binary for plugin lifecycle management (`install`, `list`, `enable`, `disable`, `remove`, `env`) |
| `plugins/csv/` | Reference CSV content-matcher plugin (Rust) |
| `plugins/protobuf/` | Reference Protobuf content-matcher plugin (JVM/Kotlin) |
| `examples/` | Cross-language integration tests (csv, protobuf, gRPC) |
| `repository/repository.index` | Checked-in plugin registry index — treat as source data, not generated output |

## Prerequisites

- `protoc` must be installed for Rust driver, CLI, JVM builds, and example/plugin builds.
- Integration tests require plugins installed in `~/.pact/plugins` (or `PACT_PLUGIN_DIR`). Install with:
  ```sh
  scripts/install-plugin-cli.sh
  ~/.pact/bin/pact-plugin-cli -y install protobuf
  ~/.pact/bin/pact-plugin-cli -y install csv
  ```
- Set `pact_do_not_track=true` to suppress plugin telemetry in local runs.

## Build, test, and lint commands

### Rust driver (`drivers/rust/driver/`)
```sh
cargo build
cargo test
cargo test <test_name>            # single test
cargo clippy --all-targets --all-features
```

### Rust CLI (`cli/`)
```sh
cargo build
cargo test
cargo test --test cli-tests cli_tests   # integration-style CLI snapshot test
cargo test <test_name>                  # single unit test
```

### Rust FFI and Pact tests
```sh
# drivers/rust/driver_ffi/
cargo test <test_name>
# drivers/rust/driver_pact_tests/
cargo test <test_name>
```

### JVM driver (`drivers/jvm/`)
```sh
./gradlew -s --no-daemon -i publishToMavenLocal
./gradlew -s --no-daemon -i check
./gradlew detekt
./gradlew :core:test --tests 'io.pact.plugins.jvm.core.DriverPactTest'   # single test
```

### CSV plugin (`plugins/csv/`)
```sh
cargo build --release
cargo test <test_name>
```

### Protobuf plugin (`plugins/protobuf/`)
```sh
./gradlew installDist
./gradlew installLocal    # unpacks into ~/.pact/plugins/protobuf-<version>
./gradlew test --tests '<fully-qualified test name>'
```

### Examples and integration checks
- **CSV**: `./gradlew check` in `examples/csv/csv-consumer-jvm`, `cargo test` in `examples/csv/csv-consumer-rust`, build provider, then verify with `~/.pact/bin/pact_verifier_cli`.
- **Protobuf**: `./gradlew check` in `examples/protobuf/protobuf-consumer-jvm`, `cargo test` in `examples/protobuf/protobuf-consumer-rust`, build/run Go provider, then verify with `~/.pact/bin/pact_verifier_cli`.
- **gRPC**: `scripts/run-grpc-examples.sh` drives `examples/gRPC/area_calculator`.

## Architecture

### Plugin protocol
Plugins communicate with drivers via gRPC using the shared contract in `proto/plugin.proto` (v1) and `proto/plugin_v2.proto` (v2, adds capability negotiation via `hostCapabilities` in `InitPluginRequest`). On startup, a plugin must print a JSON payload to stdout with `port` and optional `serverKey`; the driver reads this before any gRPC calls.

### Plugin drivers (Rust and JVM are parallel implementations)
Both drivers do: find installed plugins → load `pact-plugin.json` manifest → start plugin as a child process → read startup JSON from stdout → send `InitPluginRequest` → register catalogue entries. The feature catalogue is the key shared abstraction: entries typed as `CONTENT_MATCHER`, `CONTENT_GENERATOR`, `TRANSPORT`, `MATCHER`, or `INTERACTION`.

The Rust driver stores generated protobuf bindings checked-in at `drivers/rust/driver/src/proto.rs` and `proto_v2.rs`. `build.rs` only regenerates them when `PACT_PLUGIN_BUILD_PROTOBUFS` is set — proto changes require an explicit regeneration step.

The JVM driver is mixed-language: production code is Kotlin under `drivers/jvm/core/src/main/kotlin`, tests are Groovy/Spock specs under `drivers/jvm/core/src/test/groovy`.

### Plugin manifest (`pact-plugin.json`)
Part of the runtime contract, not just packaging metadata. Keep `entryPoint` (and optional OS-specific `entryPoints`) in sync with the actual executable layout. The Protobuf plugin demonstrates OS-specific entry points plus a JVM runtime dependency.

### Installed plugin layout
Each installed version lives in `$HOME/.pact/plugins/<name>-<version>/` containing `pact-plugin.json`. Override the root with `PACT_PLUGIN_DIR`.

### CLI and repository index
`cli/` manages plugin lifecycle; installation-by-name is driven by `repository/repository.index`. When changing installation logic or repository metadata handling, update both the CLI code and the index file.

## Rust conventions

- Rust formatting uses 2-space indentation (`rustfmt.toml`).
- Keep `lib.rs` / `main.rs` thin; most logic lives in focused modules.
- Use `anyhow` for error context propagation; avoid `unwrap()`/panic in library code.
- Existing ecosystem choices: `serde` for serialization, `tokio` for async, `tracing`/`log` for diagnostics.
- Derive `Debug`, `Clone`, `Default`, `PartialEq` where it improves ergonomics and matches surrounding code.
- Unit tests go in `mod tests` blocks; cross-module behavior tests go in `tests/`.
- Finish Rust changes with `cargo test` and `cargo clippy --all-targets --all-features`.
