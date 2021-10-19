# Simple Example Protobuf provider

This provider starts a simple HTTP server that returns the Protobuf messages based on the request body. It expects a
POST request with a JSON body, and depending on the `description` attribute will return a Protobuf message. 

To generate the Go code for the proto file, run `protoc -I=../../../proto/ --go_out=. ../../../proto/plugin.proto`

Run the provider using `go run main.go`

## Verifying the Protobuf provider

Before the provider can be verified, the Protobuf plugin needs to be built and installed into `$HOME/.pact/plugins`.
See [the plugins docs](../../../plugins/protobuf/README.md) for instructions.

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
