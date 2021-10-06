# CSV Examples

These examples demonstrate using the prototype CSV plugin to support using matching requests and responses
with CSV content. There are two consumer projects, one written in Java and the other in Rust.

The CSV provider supports the following endpoint:
* `/reports/{report}.csv`

A `GET` request will return the CSV data for the report, and a `POST` will update it.

Each consumer has two tests, one for each of the types of request to the provider.

## Running the consumer tests

Before the consumer tests can be run, the CSV plugin needs to be built and installed into `$HOME/.pact/plugins`.
See the plugins docs for instructions.

The Java consumer is run using Gradle, so just run `./gradlew check` in the `csv-consumer-jvm` directory and 
if the tests pass, a pact file will be created in the `build/pacts` directory.

The Rust consumer is run using Cargo, so just run `cargo test` in the `csv-consumer-rust` directory, and 
if the tests pass, a pact file will be created in the `target/pacts` directory.

## Verifying the CSV provider

Before the provider can be verified, the CSV plugin needs to be built and installed into `$HOME/.pact/plugins`.
See the plugins docs for instructions.

Build the provider in `csv-provider` using `cargo build`, then run the provider:

```
$ ./target/debug/csv-provider 
2021-10-06 14:09:32.631008415 [INFO] <actix_server::builder:263>:Starting 12 workers
2021-10-06 14:09:32.635307657 [INFO] <actix_server::builder:277>:Starting "actix-web-service-127.0.0.1:8080" service on 127.0.0.1:8080
```

In another terminal, use the pact_verifier_cli to verify the pacts from the consumer tests. It needs to be
version 0.9.0+ to support plugins.

```
$ pact_verifier_cli -f ../csv-consumer-rust/target/pacts/CsvClient-CsvServer.json -p 8080
03:33:17 [WARN] 

Please note:
We are tracking this plugin load anonymously to gather important usage statistics.
To disable tracking, set the 'pact_do_not_track' environment variable to 'true'.



Verifying a pact between CsvClient and CsvServer

  request for a report
    returns a response which
      has status code 200 (OK)
      includes headers
        "content-type" with value "text/csv" (OK)
      has a matching body (OK)

  request for to store a report
    returns a response which
      has status code 201 (OK)
      has a matching body (OK)
```
