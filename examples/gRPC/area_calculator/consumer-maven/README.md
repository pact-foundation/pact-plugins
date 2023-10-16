# Example Java and JUnit 5 with Maven consumer project

## Pre-Reqs

- Java 11
- Maven

## Outline

This example project has a consumer stub for the area calculator service call and exercises it in a Pact test. The
test is written in Java and JUnit 5. It tests the following interaction from the proto file:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Generated gRPC stub

The gRPC stub and Java classes are automatically generated using the Protobuf Maven plugin.

## Test class

The test class [PactConsumerTest](examples/gRPC/area_calculator/consumer-maven/src/test/java/io/pact/example/grpc/maven/PactConsumerTest.java) first sets up
the interaction using the Pact DSL, then during the test method receives a gRPC mock server to use. The generated
stub classes are then used to send the `ShapeMessage` to the mock server, and an `AreaResponse` message is received back.
This is then validated.

## Run the test

```sh
mvn test
```

A pact file, will be generated in the `./target/pacts` folder.

### Pact File

You can see it contains the Protobuf file used in the interaction, as well as the consumers request and response expectations.

These will be used to replay against the provider to verify the contract can be honoured, in isolation.

```json
{
  "consumer": {
    "name": "grpc-consumer-jvm"
  },
  "interactions": [
    {
      "comments": {
        "testname": "io.pact.example.grpc.maven.PactConsumerTest.calculateRectangleArea(MockServer, SynchronousMessages)"
      },
      "description": "calculate rectangle area request",
      "interactionMarkup": {
        "markup": "```protobuf\nmessage ShapeMessage {\n    message .area_calculator.Rectangle rectangle = 2;\n}\n```\n\n```protobuf\nmessage AreaResponse {\n    repeated float value = 1;\n}\n```\n",
        "markupType": "COMMON_MARK"
      },
      "key": "c7fbe3ee",
      "pending": false,
      "pluginConfiguration": {
        "protobuf": {
          "descriptorKey": "a85dff8f82655a9681aad113575dcfbb",
          "service": "Calculator/calculateOne"
        }
      },
      "request": {
        "contents": {
          "content": "EgoNAABAQBUAAIBA",
          "contentType": "application/protobuf; message=ShapeMessage",
          "contentTypeHint": "BINARY",
          "encoded": "base64"
        },
        "matchingRules": {
          "body": {
            "$.rectangle.length": {
              "combine": "AND",
              "matchers": [
                {
                  "match": "number"
                }
              ]
            },
            "$.rectangle.width": {
              "combine": "AND",
              "matchers": [
                {
                  "match": "number"
                }
              ]
            }
          }
        },
        "metadata": {
          "contentType": "application/protobuf;message=ShapeMessage"
        }
      },
      "response": [
        {
          "contents": {
            "content": "CgQAAEBB",
            "contentType": "application/protobuf; message=AreaResponse",
            "contentTypeHint": "BINARY",
            "encoded": "base64"
          },
          "matchingRules": {
            "body": {
              "$.value.*": {
                "combine": "AND",
                "matchers": [
                  {
                    "match": "number"
                  }
                ]
              }
            }
          },
          "metadata": {
            "contentType": "application/protobuf;message=AreaResponse"
          }
        }
      ],
      "transport": "grpc",
      "type": "Synchronous/Messages"
    }
  ],
  "metadata": {
    "pact-jvm": {
      "version": "4.4.2"
    },
    "pactSpecification": {
      "version": "4.0"
    },
    "plugins": [
      {
        "configuration": {
          "a85dff8f82655a9681aad113575dcfbb": {
            "protoDescriptors": "CsoHChVhcmVhX2NhbGN1bGF0b3IucHJvdG8SD2FyZWFfY2FsY3VsYXRvciK6AgoMU2hhcGVNZXNzYWdlEjEKBnNxdWFyZRgBIAEoCzIXLmFyZWFfY2FsY3VsYXRvci5TcXVhcmVIAFIGc3F1YXJlEjoKCXJlY3RhbmdsZRgCIAEoCzIaLmFyZWFfY2FsY3VsYXRvci5SZWN0YW5nbGVIAFIJcmVjdGFuZ2xlEjEKBmNpcmNsZRgDIAEoCzIXLmFyZWFfY2FsY3VsYXRvci5DaXJjbGVIAFIGY2lyY2xlEjcKCHRyaWFuZ2xlGAQgASgLMhkuYXJlYV9jYWxjdWxhdG9yLlRyaWFuZ2xlSABSCHRyaWFuZ2xlEkYKDXBhcmFsbGVsb2dyYW0YBSABKAsyHi5hcmVhX2NhbGN1bGF0b3IuUGFyYWxsZWxvZ3JhbUgAUg1wYXJhbGxlbG9ncmFtQgcKBXNoYXBlIikKBlNxdWFyZRIfCgtlZGdlX2xlbmd0aBgBIAEoAlIKZWRnZUxlbmd0aCI5CglSZWN0YW5nbGUSFgoGbGVuZ3RoGAEgASgCUgZsZW5ndGgSFAoFd2lkdGgYAiABKAJSBXdpZHRoIiAKBkNpcmNsZRIWCgZyYWRpdXMYASABKAJSBnJhZGl1cyJPCghUcmlhbmdsZRIVCgZlZGdlX2EYASABKAJSBWVkZ2VBEhUKBmVkZ2VfYhgCIAEoAlIFZWRnZUISFQoGZWRnZV9jGAMgASgCUgVlZGdlQyJICg1QYXJhbGxlbG9ncmFtEh8KC2Jhc2VfbGVuZ3RoGAEgASgCUgpiYXNlTGVuZ3RoEhYKBmhlaWdodBgCIAEoAlIGaGVpZ2h0IkQKC0FyZWFSZXF1ZXN0EjUKBnNoYXBlcxgBIAMoCzIdLmFyZWFfY2FsY3VsYXRvci5TaGFwZU1lc3NhZ2VSBnNoYXBlcyIkCgxBcmVhUmVzcG9uc2USFAoFdmFsdWUYASADKAJSBXZhbHVlMq0BCgpDYWxjdWxhdG9yEk4KDGNhbGN1bGF0ZU9uZRIdLmFyZWFfY2FsY3VsYXRvci5TaGFwZU1lc3NhZ2UaHS5hcmVhX2NhbGN1bGF0b3IuQXJlYVJlc3BvbnNlIgASTwoOY2FsY3VsYXRlTXVsdGkSHC5hcmVhX2NhbGN1bGF0b3IuQXJlYVJlcXVlc3QaHS5hcmVhX2NhbGN1bGF0b3IuQXJlYVJlc3BvbnNlIgBCHFoXaW8ucGFjdC9hcmVhX2NhbGN1bGF0b3LQAgFiBnByb3RvMw==",
            "protoFile": "syntax = \"proto3\";\n\npackage area_calculator;\n\noption php_generic_services = true;\noption go_package = \"io.pact/area_calculator\";\n\nservice Calculator {\n  rpc calculateOne (ShapeMessage) returns (AreaResponse) {}\n  rpc calculateMulti (AreaRequest) returns (AreaResponse) {}\n}\n\nmessage ShapeMessage {\n  oneof shape {\n    Square square = 1;\n    Rectangle rectangle = 2;\n    Circle circle = 3;\n    Triangle triangle = 4;\n    Parallelogram parallelogram = 5;\n  }\n}\n\nmessage Square {\n  float edge_length = 1;\n}\n\nmessage Rectangle {\n  float length = 1;\n  float width = 2;\n}\n\nmessage Circle {\n  float radius = 1;\n}\n\nmessage Triangle {\n  float edge_a = 1;\n  float edge_b = 2;\n  float edge_c = 3;\n}\n\nmessage Parallelogram {\n  float base_length = 1;\n  float height = 2;\n}\n\nmessage AreaRequest {\n  repeated ShapeMessage shapes = 1;\n}\n\nmessage AreaResponse {\n  repeated float value = 1;\n}\n"
          }
        },
        "name": "protobuf",
        "version": "0.3.6"
      }
    ]
  },
  "provider": {
    "name": "area-calculator-provider"
  }
}

```