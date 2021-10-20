# Protobuf Plugin prototype

This is an example plugin supporting creating and matching Protobuf messages (proto3 version).

## Building the plugin

The plugin is built with Gradle. Just run `./gradlew installDist`. This will create the plugin archive file in the 
`build/distributions` directory. There will be a Zip and Tar bundle.

## Installing the plugin

The plugin bundle and [manifest file pact-plugin.json](pact-plugin.json) need to be unpacked/copied into the `$HOME/.pact/plugins/protobuf-0.0.0` directory.
You can download the bundle and manifest from the release for the plugin.

There is also a Gradle task `installLocal` that will build the plugin and unpack it into the installation directory. 

## Example Projects

There are three example projects in [examples/protobuf](../../examples/protobuf) that use this plugin:

* protobuf-consumer - consumer written in Java
* protobuf-consumer-rust - consumer written in Rust
* protobuf-provider - provider written in Go

## Protobuf matching definitions

The plugin matches the Protobuf messages using matching rule definitions. It supports normal, repeated and map fields.

Each message needs to be configured by a map of field names to matching definitions. For instance, given the
following message:

```protobuf
message InitPluginRequest {
  string implementation = 1;
  string version = 2;
}
```

the consumer test can be configured with:

```java
builder
  .usingPlugin("protobuf")                                              // Tell pact to load the plugin for the test
  .expectsToReceive("init plugin message", "core/interaction/message")  // will use a message interaction 
  .with(Map.of(
    "message.contents", Map.of(
      "pact:proto", filePath("../../../proto/plugin.proto"),            // Need to provide the proto file
      "pact:message-type", "InitPluginRequest",                         // The message in the proto file we will be testing with
      "pact:content-type", "application/protobuf",                      // Required content type for protobuf test
      "implementation", "notEmpty('pact-jvm-driver')",                  // Require the `implementation` to not be empty (must be present and not the empty string)
      "version", "matching(semver, '0.0.0')"                            // Require the `version` field to match the semver spec
    )
  ))
  .toPact()
```

### Message fields

Fields that are messages can be matched by specifying a map for the attribute.

For example, with

```protobuf
message Body {
  string contentType = 1;
  google.protobuf.BytesValue content = 2;
  enum ContentTypeHint {
    DEFAULT = 0;
    TEXT = 1;
    BINARY = 2;
  }
  ContentTypeHint contentTypeHint = 3;
}

message InteractionResponse {
  Body contents = 1;
}
```

the consumer test can be configured with:

```java
builder
    .usingPlugin("protobuf")
    .expectsToReceive("Configure Interaction Response", "core/interaction/message")
    .with(Map.of(
        "message.contents", Map.of(
          "pact:proto", filePath("../../../proto/plugin.proto"),
          "pact:message-type", "InteractionResponse",
          "pact:content-type", "application/protobuf",
          "contents", Map.of(                                               // contents is a message, so use a map to confugure the matching
            "contentType", "notEmpty('application/json')",                  // contents.contentType must not be empty
            "content", "matching(contentType, 'application/json', '{}')",   // contents.content must contain JSON data
            "contentTypeHint", "matching(equalTo, 'TEXT')"                  // contents.contentTypeHint must be equal to TEXT (enum value)
          )
        )
    ))
    .toPact();
```

### Map and repeated fields

Map and repeated fields can be specified using a similar mechanism, but need a `pact:match` entry that configures
how each item in the collection can be matched.

For example, given the following messages:

```protobuf
message MatchingRule {
  string type = 1;
  google.protobuf.Struct values = 2;
}

message MatchingRules {
  repeated MatchingRule rule = 1;
}

message InteractionResponse {
  map<string, MatchingRules> rules = 2;
}
```

you can configure the matching with 

```java
"rules", Map.of(
    // Match each key in the map using a regex, and each item must match by type 
    // (the example will come from the map, so we can use null here)
    "pact:match", "eachKey(matching(regex, '\\$(\\.\\w+)+', '$.test.one')), eachValue(matching(type, null))",
    // This is the example map entry to use for matching
    "$.test.one", Map.of(
      "rule", Map.of(
        // rule is a repeated field, so we define an "eachValue" matcher to match the item defined by "items"
        "pact:match", "eachValue(matching($'items'))",
        // the example to match each item in the "rule" collection
        "items", Map.of(
          "type", "notEmpty('regex')" // each item in the "rule" collection must have a "type" field that is not empty
        )
      )
)
```

## Verifying the provider

Verifying the provider just works as a normal message pact verification. In the provider example, there is a Go
HTTP server that returns the Protobuf message based on the interaction description. Pointing the pact_verifier_cli 
at it to verify the pacts from the consumer tests works as normal. It needs to be version 0.9.0+ to support plugins. 
