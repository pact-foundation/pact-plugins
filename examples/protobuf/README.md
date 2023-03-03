# Protobuf Examples

These examples demonstrate using the prototype Protobuf plugin to support matching Protobuf messages. 
There are two consumer projects, one written in Java and the other in Rust.

The [proto file](../../interfaces/proto/plugin.proto) for the plugin interface is used for these tests.  

Each consumer has two tests, one for the simple InitPluginResponse message and one for the more complex
InteractionResponse message.

## Running the consumer tests

Before the consumer tests can be run, the Protobuf plugin needs to be built and installed into `$HOME/.pact/plugins`.
See [the plugins docs](../../plugins/protobuf/README.md) for instructions.

The Java consumer is run using Gradle, so just run `./gradlew check` in the `protobuf-consumer` directory and 
if the tests pass, a pact file will be created in the `build/pacts` directory.

The Rust consumer is run using Cargo, so just run `cargo test` in the `protobuf-consumer-rust` directory, and 
if the tests pass, a pact file will be created in the `target/pacts` directory.

## Verifying the Protobuf provider

Before the provider can be verified, the Protobuf plugin needs to be built and installed into `$HOME/.pact/plugins`.
See [the plugins docs](../../plugins/protobuf/README.md) for instructions.

Run the provider using `go run main.go`

In another terminal, use the pact_verifier_cli to verify the pacts from the consumer tests. It needs to be
version 0.9.0+ to support plugins. The provider will be running on port 8111.

```
$ pact_verifier_cli -f ../protobuf-consumer-jvm/build/pacts/protobuf-consumer-protobuf-provider.json -p 8111
05:56:37 [WARN] 

Please note:
We are tracking this plugin load anonymously to gather important usage statistics.
To disable tracking, set the 'pact_do_not_track' environment variable to 'true'.



Verifying a pact between protobuf-consumer and protobuf-provider

  Configure Interaction Response

  Test Name: io.pact.example.protobuf.provider.PactConsumerTest.consumeConfigureInteractionResponseMessage(AsynchronousMessage)
    generates a message which
      has a matching body (OK)

  init plugin message

  Test Name: io.pact.example.protobuf.provider.PactConsumerTest.consumeInitPluginMessage(AsynchronousMessage)
    generates a message which
      has a matching body (OK)


```
