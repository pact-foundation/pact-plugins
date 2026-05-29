# JSON-RPC sum calculator example

This example demonstrates the `jsonrpc` transport plugin with a small JSON-RPC 2.0 over HTTP service.

- `consumer-rust/` creates a Pact by talking to a plugin-backed mock server
- `provider-rust/` exposes a `sum` JSON-RPC method and includes an opt-in verifier test

## Prerequisites

- `cargo`
- `just` if you want to use the helper recipes
- a locally installed `jsonrpc` plugin in `~/.pact/plugins/jsonrpc-0.1.0-beta.1`
- a verifier build with v2 plugin support for the ignored provider verification test
- `pact_do_not_track=true` if you want to suppress telemetry during local runs

## Helper recipes

```sh
just consumer
just provider
just all
```

## Run the consumer test

```sh
cd consumer-rust
cargo test
```

That writes a Pact file to `consumer-rust/pacts/`.

## Run the provider verifier test

```sh
cd provider-rust
cargo test -- --ignored verify_jsonrpc_provider
```

The provider verification test is intentionally ignored by default until the verifier used in your environment supports the v2 plugin gRPC interface.
