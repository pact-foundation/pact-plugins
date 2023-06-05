# Examples dealing with gRPC error responses

These examples take the [Area Calculator Example](../area_calculator) and modify it to return an unimplemented
response for one of the shapes. There are two consumer projects (one in Java and one in Rust), and the same for the
providers. They are setup to test sending the unimplemented shape message and assert that the correct gRPC status and
message is returned.

* [Java Consumer](consumer-jvm)
* [Rust consumer](consumer-rust)
* [Java Provider](provider-jvm)
* [Rust Provider](provider-rust)
