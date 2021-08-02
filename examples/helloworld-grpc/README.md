# Hello Example

This is the hello example from the GRPC getting started using a Java client talking to a Go server.

## Requirements
* Go compiler
* protoc compiler
* JVM

## Running the server

In the server directory, run 

```console
$ protoc --proto_path ../proto helloworld.proto --go_out=proto --go_opt=paths=source_relative --go-grpc_out=proto --go-grpc_opt=paths=source_relative
$ go run main.go 
```

## Running the client

In the client directory, run

```console
$ ./gradlew installDist
$ ./build/install/hello-world-client/bin/hello-world-client
```
