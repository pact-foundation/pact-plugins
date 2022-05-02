# Area Calculator Example

This is a simple gRPC example that can receive a shape via gRPC, and return the area for the shape. See the [proto file](proto/area_calculator.proto)
for more details.

The proto file has a single service method which these examples will be testing:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

## Java/JUnit 5 consumer

The example [JVM consumer project](consumer-jvm) contains a simple consumer in Java generated from Gradle and a JUnit 5 consumer test.

## Rust consumer

The example [Rust consumer project](consumer-rust) contains a simple consumer generated with Prost and a Rust consumer test.

## Kotlin Provider and Java/JUnit 5 test

The [provider project](provider-jvm) contains a Kotlin server and a Java/JUnit 5 test to verify the Pact file from the consumer projects.
It can also be verified using the Rust verifier CLI.
