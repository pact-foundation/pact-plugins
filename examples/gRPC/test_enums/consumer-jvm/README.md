# Example Java and JUnit 5 consumer project

This example project has a consumer stub for the area calculator service call and exercises it in a Pact test. The
test is written in Java and JUnit 5. It tests the following interaction from the proto file:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Generated gRPC stub

The gRPC stub and Java classes are automatically generated using the Protobuf Gradle plugin.

## Test class

The test class [PactConsumerTest](src/test/java/io/pact/example/grpc/consumer/PactConsumerTest.java) first sets up
the interaction using the Pact DSL, then during the test method receives a gRPC mock server to use. The generated
stub classes are then used to send the `ShapeMessage` to the mock server, and an `AreaResponse` message is received back.
This is then validated.
