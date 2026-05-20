# Copilot instructions for `pact-plugins`

## Build, test, and lint commands

This repository is **not** a single top-level workspace. Run commands in the component you are changing.

### Common prerequisites

- `protoc` must be installed for the Rust driver, CLI workflow, JVM builds, and the example/plugin builds.
- Driver and example flows often require installed plugins in `~/.pact/plugins` (or `PACT_PLUGIN_DIR`). CI installs the CLI with `scripts/install-plugin-cli.sh` and then installs plugins with `~/.pact/bin/pact-plugin-cli -y install protobuf` and, where needed, `~/.pact/bin/pact-plugin-cli -y install csv`.
- To suppress plugin telemetry in local runs, use `pact_do_not_track=true`.

### Main components

| Area | Build / full test / lint | Single-test pattern |
| --- | --- | --- |
| `cli/` | `cargo build`<br>`cargo test` | `cargo test --test cli-tests cli_tests` for the integration-style CLI snapshot test, or `cargo test <test_name>` for a specific unit test |
| `drivers/rust/driver/` | `cargo build`<br>`cargo test`<br>`cargo clippy --all-targets --all-features` | `cargo test <test_name>` |
| `drivers/rust/driver_ffi/` | `cargo test` | `cargo test <test_name>` |
| `drivers/rust/driver_pact_tests/` | `cargo test` | `cargo test <test_name>` |
| `drivers/jvm/` | `./gradlew -s --no-daemon -i publishToMavenLocal`<br>`./gradlew -s --no-daemon -i check`<br>`./gradlew detekt` | `./gradlew :core:test --tests 'io.pact.plugins.jvm.core.DriverPactTest'` or another fully-qualified test/spec name |
| `plugins/csv/` | `cargo build --release` | `cargo test <test_name>` if you add or update Rust tests there |
| `plugins/protobuf/` | `./gradlew installDist`<br>`./gradlew installLocal` to unpack directly into `~/.pact/plugins/protobuf-<version>` | `./gradlew test --tests '<fully-qualified test name>'` |

### Examples and integration checks

- `examples/csv/` is exercised the same way as CI: run `./gradlew check` in `csv-consumer-jvm`, `cargo test` in `csv-consumer-rust`, build the provider with `cargo build`, then verify the produced pacts with `~/.pact/bin/pact_verifier_cli`.
- `examples/protobuf/` is the main cross-language protobuf/grpc smoke suite: run `./gradlew check` in `protobuf-consumer-jvm`, `cargo test` in `protobuf-consumer-rust`, build/run the Go provider, then verify with `~/.pact/bin/pact_verifier_cli`.
- `examples/gRPC/area_calculator` is the main grpc end-to-end workflow; CI drives it with `scripts/run-grpc-examples.sh`.

## High-level architecture

- `proto/plugin.proto` is the shared Pact plugin gRPC contract. Both driver implementations and the reference plugins are built around this file. The Rust driver and CSV plugin compile/generate bindings from it in `build.rs`; the Protobuf plugin’s Gradle build generates Java/Kotlin gRPC sources from the same shared proto directory.
- `drivers/rust/driver/` and `drivers/jvm/core/` are parallel implementations of the same plugin-driver responsibilities: find installed plugins, load `pact-plugin.json`, start the plugin as a child process, read the startup JSON from stdout (`port` + `serverKey`), send `InitPluginRequest`, and register the plugin’s catalogue entries so Pact can delegate content matching, generation, transports, mock servers, and verification to plugins.
- The Rust side is split into three crates: `driver/` (core driver logic), `driver_ffi/` (FFI-facing tests around the Rust driver), and `driver_pact_tests/` (Pact-based contract tests for the driver itself).
- `cli/` is a separate Rust binary for plugin lifecycle management (`install`, `list`, `enable`, `disable`, `remove`, `env`). Installation-by-name is driven by the checked-in index in `repository/repository.index`.
- `plugins/csv/` and `plugins/protobuf/` are reference implementations of the plugin contract, while `examples/` is the main integration surface that proves consumers, providers, drivers, and installed plugins interoperate across languages.

## Key conventions

- Installed plugins live in `$HOME/.pact/plugins` by default, or `PACT_PLUGIN_DIR` when overridden. Each installed version gets its own `<name>-<version>` directory containing a `pact-plugin.json` manifest.
- The manifest is part of the runtime contract, not just packaging metadata. Keep `pact-plugin.json` in sync with the executable layout (`entryPoint`, optional OS-specific `entryPoints`, dependency declarations). The Protobuf plugin is the main example of an OS-specific entry point plus a runtime dependency on a JVM.
- Plugins are expected to behave like stateless child processes. On startup they must print a small JSON payload to stdout with the gRPC `port` and optional `serverKey`; the drivers rely on that handshake before any gRPC calls happen.
- The feature catalogue is the key abstraction shared across drivers and plugins. When touching matching, generation, transport, or interaction support, check how the change affects catalogue registration and lookup, not just the immediate code path.
- The Rust driver intentionally checks in generated protobuf bindings in `drivers/rust/driver/src/proto.rs`. `drivers/rust/driver/build.rs` only regenerates them when `PACT_PLUGIN_BUILD_PROTOBUFS` is set, so proto changes usually require an explicit regeneration step rather than assuming build-time codegen always runs.
- The JVM driver is mixed-language: production code is mainly Kotlin under `drivers/jvm/core/src/main/kotlin`, but many tests are Groovy/Spock specs under `drivers/jvm/core/src/test/groovy`. Do not assume JVM tests are all JUnit/Kotlin.
- If you change plugin installation-by-name or repository metadata handling, update both the CLI logic and `repository/repository.index`; that index is treated as checked-in source data, not generated output.

### Rust conventions

- Keep `lib.rs` and `main.rs` thin. Follow the existing crate layout where orchestration lives in the entry file and most logic lives in focused modules.
- Prefer idiomatic error propagation with `Result` and `?`, and add context with crates already used here such as `anyhow`. Avoid introducing new `unwrap()`/panic paths in library code unless the invariant is truly internal and fixed.
- Reuse the repo’s existing Rust ecosystem choices: `serde` for serialization, `tokio` for async work, and `tracing`/`log` for diagnostics instead of ad hoc patterns.
- Prefer borrowing, iterators, and smaller helper functions over cloning, index-heavy loops, or deeply nested control flow.
- Derive common traits like `Debug`, `Clone`, `Default`, and `PartialEq` when they materially improve ergonomics and match the surrounding code.
- Keep Rust tests close to the code when practical with `mod tests`, and use crate-level integration tests in `tests/` when behavior crosses module boundaries.
- For Rust changes, finish with the component’s `cargo test` and, where available, `cargo clippy --all-targets --all-features`.
