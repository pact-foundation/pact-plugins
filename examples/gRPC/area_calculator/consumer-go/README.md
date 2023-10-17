# Example Go consumer project

This example project has a consumer stub for the area calculator service call and exercises it in a Pact test.
It tests the following interaction from the proto file:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

You need to [install the Pact Go library](https://github.com/pact-foundation/pact-go/tree/2.x.x#installation).

For more information about gRPC in Go - Check out the [Go gRPC quick start](https://grpc.io/docs/languages/go/quickstart/)

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Generated gRPC stub

To generate the Go code for the proto file, you need to install the Protobuf compiler (protoc), and install the Go
protobuf and grpc protoc plugins, and then run `protoc --go_out=. --go-grpc_out=.  --proto_path ../proto  ../proto/area_calculator.proto`.

### Protoc Compiler

- https://pkg.go.dev/google.golang.org/protobuf/cmd/protoc-gen-go
- https://pkg.go.dev/google.golang.org/grpc/cmd/protoc-gen-go-grpc

```sh
go install google.golang.org/protobuf/cmd/protoc-gen-go@v1.31.0
go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@v1.3.0
export PATH="$PATH:$(go env GOPATH)/bin"
protoc --go_out=. --go-grpc_out=.  --proto_path ../proto  ../proto/area_calculator.proto
```

## Test method

The test method [TestCalculateClient](consumer_test.go) first sets up the interaction using the Pact DSL, then sets up a
gRPC mock server to use. The generated stub structs are then used to send the `ShapeMessage` to the mock server,
and an `AreaResponse` message is received back. This is then validated.

## Run the test

```sh
go test
```

A pact file, will be generated in the `./pacts` folder.

### Pact File

You can see it contains the Protobuf file used in the interaction, as well as the consumers request and response expectations.

These will be used to replay against the provider to verify the contract can be honoured, in isolation.

```json
{
  "consumer": {
    "name": "grpc-consumer-go"
  },
  "interactions": [
    {
      "description": "calculate rectangle area request",
      "interactionMarkup": {
        "markup": "```protobuf\nmessage AreaResponse {\n    repeated float value = 1;\n}\n```\n",
        "markupType": "COMMON_MARK"
      },
      "pending": false,
      "pluginConfiguration": {
        "protobuf": {
          "descriptorKey": "a85dff8f82655a9681aad113575dcfbb",
          "service": "Calculator/calculateMulti"
        }
      },
      "request": {
        "contents": {
          "content": "CgwSCg0AAEBAFQAAgEAKBwoFDQAAQEA=",
          "contentType": "application/protobuf;message=AreaRequest",
          "contentTypeHint": "BINARY",
          "encoded": "base64"
        },
        "matchingRules": {
          "body": {
            "$.shapes[0].rectangle.length": {
              "combine": "AND",
              "matchers": [
                {
                  "match": "number"
                }
              ]
            },
            "$.shapes[0].rectangle.width": {
              "combine": "AND",
              "matchers": [
                {
                  "match": "number"
                }
              ]
            },
            "$.shapes[1].square.edge_length": {
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
          "contentType": "application/protobuf;message=AreaRequest"
        }
      },
      "response": [
        {
          "contents": {
            "content": "CggAAEBBAAAQQQ==",
            "contentType": "application/protobuf;message=AreaResponse",
            "contentTypeHint": "BINARY",
            "encoded": "base64"
          },
          "matchingRules": {
            "body": {
              "$.value[0].*": {
                "combine": "AND",
                "matchers": [
                  {
                    "match": "number"
                  }
                ]
              },
              "$.value[1].*": {
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
    "pactRust": {
      "ffi": "0.4.5",
      "mockserver": "1.1.1",
      "models": "1.1.2"
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

## Run the consumer & provider

In terminal 1: Start the provider

```sh
cd ../provider-go
go run provider.go
```

You should see an address

```console
go run provider.go
2023/10/16 21:39:17 Server started 127.0.0.1:58132
```

In terminal 2: Run the consumer

Use the address, shown by the provider, with the `--addr` flag

```sh
cd consumer-go
go run consumer.go --addr localhost:58132
```

Outgoing request

```console
2023/10/16 21:40:39 Sending calculate rectangle and square request
```

You should see the provider receive the request

```console
2023/10/16 21:40:39 Calculating the area for multiple values shapes:<rectangle:<length:3 width:4 > > shapes:<square:<edge_length:3 > > 
```

And the consumer showing the returned response

```console
2023/10/16 21:40:39 Sending calculate rectangle and square request
Areas: [12.000000 9.000000]
```
