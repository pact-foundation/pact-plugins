# Example Maven provider project

This example project has a server implementation in Kotlin for the area calculator service call:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

The Maven Protobuf plugin is used to generate the gRPC classes for the calculate service call and the [Kotlin Server
class](server/src/main/kotlin/io/pact/example/grpc/provider/Server.kt) implements the calculate method.

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Pact verification test

There is a [Pact verification test](server/src/test/java/io/pact/example/grpc/provider/PactVerificationTest.java) 
written in Java and JUint 5 that can verify the Kotlin server using a Pact file from one of the consumer projects.

In order to run the test, you must copy the pact files, from the consumer directory

```sh
mkdir -p src/test/resources/pacts
cp ../consumer-maven/target/pacts/*.json src/test/resources/pacts
```

You can then run the provider tests with

```sh
mvn test
```
