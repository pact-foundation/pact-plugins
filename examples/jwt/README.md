# JWT Examples

These examples demonstrate using the [JWT plugin](../../plugins/jwt) (written in Lua) to support requests and
responses signed as JSON Web Tokens. There are two consumer projects, one written in Rust and the other in Java,
and one provider, written in Rust.

They also demonstrate a plugin **delegating a matching rule back to the host Pact framework**. Both consumer tests
declare a `customer_id` claim with a `regex` rule rather than a fixed value:

```json
"customer_id": { "pact:matcher:type": "regex", "regex": "CUST-\\d{6}", "value": "CUST-123456" }
```

The JWT plugin implements no matching rules of its own - it decodes and validates the token, then hands that one
claim to whoever owns `regex` in the running catalogue, which is the Pact framework doing the verifying (see
proposals [006](../../docs/proposals/006_Field_level_matchers_and_generators.md) and
[009](../../docs/proposals/009_Host_provided_core_matching_and_generation.md)). It is worth checking that this is
really doing something: the token the consumer test's client sends carries `CUST-987654`, and the provider issues
a different customer id on every request, so neither would match the example value under the plugin's usual
equality check. Change the client's value to something outside the pattern and the test fails with the framework's
own message, `Expected 'not-a-customer' to match 'CUST-\d{6}'`.

The provider supports one endpoint:
* `POST /token` - returns a freshly-signed JWT.

## Running the consumer tests

Before the consumer tests can be run, the JWT plugin needs to be installed into `$HOME/.pact/plugins` - there's no
build step, just run [`install-local.sh`](../../plugins/jwt/install-local.sh) in the [plugins/jwt](../../plugins/jwt)
directory. See [the plugin docs](../../plugins/jwt/README.md) for details.

You'll also need a driver build that includes Lua plugin support - see
[Using a Lua-plugin-capable driver build](#using-a-lua-plugin-capable-driver-build) below, since (as of this
writing) that isn't in a released version yet.

The Rust consumer is run using Cargo, so just run `cargo test` in the `jwt-consumer-rust` directory, and if the
test passes, a pact file will be created in `target/pacts/JwtClient-JwtServer.json`.

The Java consumer is run using Gradle, so just run `./gradlew test` in the `jwt-consumer-jvm` directory, and if the
test passes, a pact file will be created in `build/pacts/JwtClient-JwtServer.json`.

## Verifying the JWT provider

Build and run the provider in `jwt-provider-rust`:

```console
$ cargo build
$ ./target/debug/jwt-provider-rust
```

This starts an HTTP server on `127.0.0.1:8080`.

In another terminal, verify one of the pact files generated above against it. As with the consumer tests, this
needs a Lua-plugin-capable verifier - see the next section for how to build one locally.

```console
$ ./pact-verifier -f ../jwt-consumer-rust/target/pacts/JwtClient-JwtServer.json --hostname 127.0.0.1 --port 8080

Verifying a pact between JwtClient and JwtServer

  request for a token exchange (0s loading, 21ms verification)
    returns a response which
      has status code 200 (OK)
      includes headers
        "content-type" with value "application/jwt+json" (OK)
      has a matching body (OK)
```

## Using a Lua-plugin-capable driver build

As of this writing, Lua plugin support hasn't shipped in a released `pact-plugin-driver`/
`io.pact.plugin.driver:core` version yet, so both the consumer tests above and `pact_verifier_cli` need to be
pointed at a local build of this repo's `lua-plugins` branch (or wherever this work has landed by the time you're
reading this).

**Consumer tests** already handle this via `[patch.crates-io]` in each project's `Cargo.toml` (Rust) or
`mavenLocal()` (JVM, once you've run `./gradlew publishToMavenLocal` in `drivers/jvm`) - no extra steps needed
beyond building the driver itself.

**Provider verification** needs a `pact-verifier` binary built against the patched driver. If you have a local
checkout of [pact-reference](https://github.com/pact-foundation/pact-reference):

1. Add a `[patch.crates-io]` entry to `rust/Cargo.toml` in your `pact-reference` checkout, pointing at your local
   `pact-plugins` checkout:
   ```toml
   [patch.crates-io]
   pact-plugin-driver = { path = "/path/to/pact-plugins/drivers/rust/driver" }
   ```
2. `pact_verifier`'s own dependency on `pact-plugin-driver` is `default-features = false` and doesn't request the
   `lua` feature, so add an explicit dependency in `rust/pact_verifier_cli/Cargo.toml` to turn it on:
   ```toml
   pact-plugin-driver = { version = "~1.2.2", default-features = false, features = ["lua"] }
   ```
3. Build it:
   ```console
   $ cargo build -p pact_verifier_cli
   ```
   The binary ends up at `target/debug/pact-verifier` (hyphenated, not `pact_verifier_cli`).

The `customer_id` claim's `regex` rule additionally needs a host that registers the standard matching rules as
core catalogue capabilities - `pact_matching` 2.0.11+ (so a locally-built `pact_verifier_cli` picks it up
automatically) and Pact-JVM 4.7.5+. Against an older host the claim is reported as a mismatch saying the rule
could not be resolved, rather than being silently skipped.

Once Lua plugin support is released, none of the above will be necessary - just use the normal released
`pact_verifier_cli`/pact-jvm provider verifier.

There is currently no `jwt-provider-jvm` example, so there's no way to verify the JVM driver's provider-side
support directly this way - but it's exercised indirectly, since the JVM driver's `compareContents`/
`generateContent` Lua bridging is unit-tested directly (see `LuaPactPluginTest` in `drivers/jvm/core`) and is the
same code path a provider verification would use.
