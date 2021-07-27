# Hello Example

This is the hello example from the GRPC getting started using a Java client talking to a Go server.

## Running the server

In the server directory, run 

```console
$ go run main.go 
```

## Running the client

In the client directory, run

```console
$ ./gradlew installDist
$ ./build/install/hello-world-client/bin/hello-world-client
```

