# Example Go consumer project

This example project has a consumer stub for the area calculator service call and exercises it in a Pact test.
It tests the following interaction from the proto file:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

You need to [install the Pact Go library](https://github.com/pact-foundation/pact-go/tree/2.x.x#installation). 

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Generated gRPC stub

To generate the Go code for the proto file, you need to install the Protobuf compiler (protoc), and install the Go
protobuf and grpc protoc plugins, and then run `protoc --go_out=. --go-grpc_out=.  --proto_path ../proto  ../proto/area_calculator.proto`.

## Test method

The test method [TestCalculateClient](consumer_test.go) first sets up the interaction using the Pact DSL, then sets up a
gRPC mock server to use. The generated stub structs are then used to send the `ShapeMessage` to the mock server,
and an `AreaResponse` message is received back. This is then validated.
