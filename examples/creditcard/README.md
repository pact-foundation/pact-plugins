# Credit card examples

These examples demonstrate the [creditcard plugin](../../plugins/creditcard) (written in Lua), which provides a
**field-level** matching rule and generator rather than owning a content type.

That is what makes these examples different from the [CSV](../csv), [Protobuf](../protobuf) and [JWT](../jwt) ones:
the body here is ordinary JSON, matched by the core JSON matcher, and the plugin contributes one rule and one
generator that apply to a single value inside it. See
[proposal 006, Field-level matchers and generators](../../docs/proposals/006_Field_level_matchers_and_generators.md).

There are two consumer projects, one written in Rust and the other in Java, and one provider written in Rust that
both verify against.

The provider supports one endpoint:
* `GET /cards/{brand}` - returns a card of that brand: `{"card": {"number": …, "expiry": …, "brand": …}}`

## What the examples show

The consumer asserts that `$.card.number` is a plausible Visa number, which is not expressible with any core
matching rule - `regex` can check the shape of a card number but not its check digit:

```json
"matchingRules": {
  "body": {
    "$.card.number": {
      "combine": "AND",
      "matchers": [ { "match": "creditcard", "brand": "visa" } ]
    }
  }
},
"generators": {
  "body": {
    "$.card.number": { "type": "creditcard", "brand": "visa" }
  }
}
```

Note there is no marker in the Pact file saying either of these comes from a plugin - which plugin (if any)
provides a name is a property of the running catalogue, not of the file. What *is* recorded is a `plugins` entry in
the file's metadata, so provider verification knows it has to load the `creditcard` plugin before it can interpret
the file.

The generator is exercised too: the mock server answers with a freshly generated Luhn-valid Visa number on every
run, rather than the example value in the Pact file.

## Running the consumer tests

First install the plugin into `$HOME/.pact/plugins`. There's no build step - it's plain Lua - so just run
[`install-local.sh`](../../plugins/creditcard/install-local.sh) in the
[plugins/creditcard](../../plugins/creditcard) directory.

The Rust consumer is run with Cargo, so run `cargo test` in `creditcard-consumer-rust`. If the test passes, a pact
file is written to `target/pacts/CreditCardClient-CreditCardServer.json`.

The Java consumer is run with Gradle, so run `./gradlew test` in `creditcard-consumer-jvm`. If the test passes, a
pact file is written to `build/pacts/CreditCardClient-CreditCardServer.json`.

## Verifying the provider

Build and run the provider in `creditcard-provider-rust`:

```console
$ cargo build
$ ./target/debug/creditcard-provider-rust
```

This starts an HTTP server on `127.0.0.1:8080`.

In another terminal, verify one of the pact files generated above against it:

```console
$ pact_verifier_cli -f ../creditcard-consumer-rust/target/pacts/CreditCardClient-CreditCardServer.json -p 8080
```

To see the rule actually bite, change the Visa number in
[`creditcard-provider-rust/src/main.rs`](creditcard-provider-rust/src/main.rs) to one with a bad check digit
(`4111111111111112`) and verify again. The plugin's own message comes back against the field's path:

```
1.1) has a matching body
       $.card.number -> Expected a credit card number, but '4111111111111112' fails the Luhn check
                        (its last digit is not a valid check digit)
```

## Compatibility

Field-level matching rules and generators need:

* a plugin driver that implements them (`pact-plugin-driver` 1.2.0 / `io.pact.plugin.driver:core` 1.2.0 or later),
  built with Lua plugin support enabled;
* a host Pact framework that can carry a plugin-provided rule - `pact_models`/`pact_matching`/`pact_consumer` on the
  Rust side, and the model and matchers modules on the Pact-JVM side.

`pact_verifier_cli` needs to be built with the driver's `lua` feature turned on to load this plugin at all; the
[JWT example README](../jwt/README.md#using-a-lua-plugin-capable-driver-build) covers how to do that from a local
checkout.

> [!NOTE]
> These pieces have to be released in order: `pact_models` first (it adds `value_map()`, which is how a plugin
> rule's configuration values reach the plugin at all), then `pact-plugin-driver`, then `pact_matching` /
> `pact_consumer` / `pact_ffi`. Against an older driver the rule still resolves and runs, but its configuration
> arrives empty - so a `brand`-constrained rule would accept any well-formed card number. Pact-JVM is independent
> of that chain.
