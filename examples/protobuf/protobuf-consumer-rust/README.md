# Protobuf Rust Consumer Example

This example demonstrates using the prototype Protobuf plugin to support matching Protobuf messages.

The [proto file](../../../proto/plugin.proto) for the plugin interface is used for these tests.  

The consumer has two tests, one for the simple InitPluginResponse message and one for the more complex
InteractionResponse message.

## Running the consumer tests

Before the consumer tests can be run, the Protobuf plugin needs to be built and installed into `$HOME/.pact/plugins`.
See [the plugins docs](../../../plugins/protobuf/README.md) for instructions.

The Rust consumer is run using Cargo, so just run `cargo test`, and 
if the tests pass, a pact file will be created in the `target/pacts` directory.
