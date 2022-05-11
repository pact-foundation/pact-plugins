# Example Rust consumer project

This example project has a consumer stub for the area calculator service call and exercises it in a Pact test. 
It tests the following interaction from the proto file:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Generated gRPC stub

The gRPC structs are automatically generated using Prost in the build script.

## Test method

The test method [test_proto_client](src/lib.rs) first sets up the interaction using the Pact DSL, then sets up a 
gRPC mock server to use. The generated stub structs are then used to send the `ShapeMessage` to the mock server, 
and an `AreaResponse` message is received back. This is then validated.
