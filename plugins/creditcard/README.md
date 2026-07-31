# Credit card plugin (written in Lua)

An example plugin providing a **field-level** matching rule and generator for credit card numbers. Unlike the
[CSV](../csv), [Protobuf](../protobuf) and [JWT](../jwt) plugins, it does not own a content type at all - it
contributes one rule that applies to a single value inside somebody else's content: a field in a JSON body, a
header, a message metadata value, and so on.

It is the reference implementation for
[proposal 006, Field-level matchers and generators](../../docs/proposals/006_Field_level_matchers_and_generators.md),
and was written alongside that design to keep it honest. Like the [JWT plugin](../jwt), it is written in Lua and
runs embedded in the driver's own process rather than as a separate gRPC server - see
[Writing plugins in Lua](../../docs/writing-plugin-guide.md#writing-plugins-in-lua).

> [!IMPORTANT]
> Field-level matchers and generators are **not implemented in either driver yet** - proposal 006 is still a draft.
> This plugin cannot be loaded and run against a real test today. Until it can, [`test.lua`](test.lua) exercises it
> directly. See [Compatibility](#compatibility) below.

## What it does

The plugin registers two catalogue entries, both named `creditcard`:

| Entry type | What it does |
|---|---|
| `MATCHER` | Asserts the actual value is a plausible credit card number |
| `GENERATOR` | Produces a fresh, valid credit card number on each test run |

The matcher checks, in order:

1. The value is text (or an integer) - binary data and other types are a mismatch.
2. After removing spaces and dashes, it is all digits.
3. Its last digit is a valid [Luhn](https://en.wikipedia.org/wiki/Luhn_algorithm) check digit. This is the part no
   core Pact matching rule can express: `regex` can check the shape of a card number, but not its checksum.
4. If the rule is configured with a `brand`, the number starts with an issuer identification number used by that
   brand and has a length that brand issues. If no `brand` is configured, any well-formed number of 12 to 19 digits
   is accepted, whichever issuer it belongs to.

Known brands: `amex`, `diners`, `discover`, `jcb`, `mastercard`, `visa`. Configuring a brand the plugin doesn't know
is reported as an *error* rather than a mismatch - it's a mistake in the test, not in the provider's response.

The generator produces a number for the configured `brand`; with no brand configured it uses whichever brand the
example value in the Pact file looks like, falling back to Visa. It is a pure function of the request - the brand,
the example value, and the test context are all it reads.

## Using it in a test

Set the rule up on a field the same way as any core matching rule, by name:

```json
{
  "card": {
    "number": "matching(creditcard, 'visa', '4111111111111111')",
    "expiry": "matching(regex, '\\d{2}/\\d{2}', '04/28')"
  }
}
```

The optional middle argument is the brand - the plugin declares `config-key = "brand"` on its catalogue entries,
which is what maps that positional argument onto the rule's `brand` value. The equivalent JSON form, which can carry
any number of configuration values and can attach the generator too, is:

```json
{
  "pact:matcher:type": "creditcard",
  "pact:generator:type": "creditcard",
  "brand": "visa",
  "value": "4111111111111111"
}
```

Either way it is persisted into the Pact file as an ordinary matching rule whose name happens to come from a plugin:

```json
"matchingRules": {
  "body": {
    "$.card.number": {
      "combine": "AND",
      "matchers": [ { "match": "creditcard", "brand": "visa" } ]
    }
  }
}
```

The consumer test has to load the plugin (`usingPlugin("creditcard")` or your framework's equivalent) before the
rule can be resolved.

## Files

- [`plugin.lua`](plugin.lua) - the entry point (see `entryPoint` in [`pact-plugin.json`](pact-plugin.json)).
  Defines `init`, `match_field` and `generate_field`.
- [`creditcard.lua`](creditcard.lua) - the card number logic itself: normalisation, the Luhn checksum, per-brand IIN
  prefixes and lengths, brand detection and number generation. Knows nothing about Pact.
- [`test.lua`](test.lua) - a standalone test harness that stands in for the driver.

There are no vendored third-party libraries - everything the plugin needs is in Lua's standard library.

## Testing

```console
$ lua5.4 test.lua
ok   - init returns two catalogue entries
...
All checks passed
```

The harness stubs the `logger` host function, loads `plugin.lua`, and calls its global functions with the same table
shapes the driver builds. It covers each scheme's published test numbers, the mismatch cases, and round-trips every
generated number back through the matcher.

## Installing the plugin

There's no build step - it's plain Lua files. Run [`install-local.sh`](install-local.sh), which copies the plugin
into `$PACT_PLUGIN_DIR/creditcard-0.0.0/` (or `~/.pact/plugins/creditcard-0.0.0/` if `PACT_PLUGIN_DIR` isn't set):

```console
$ ./install-local.sh
Installed the creditcard plugin into /home/you/.pact/plugins/creditcard-0.0.0
```

## Compatibility

Requires a driver that implements field-level matchers and generators
([proposal 006](../../docs/proposals/006_Field_level_matchers_and_generators.md)), and a host Pact framework that can
resolve a plugin-provided matching rule. Neither exists yet - the proposal's sequencing section tracks what has to
land first:

- the `GENERATOR` catalogue entry type and the `MatchField`/`GenerateField` operations in `proto/plugin_v2.proto`;
- the field matcher/generator surface in the Rust and JVM drivers, including the Lua `match_field`/`generate_field`
  entry points this plugin defines;
- carrier variants for plugin-provided rules and generators in `pact_models` and the Pact-JVM model, plus dispatch
  from the matching engines.

Until then this plugin is a design artefact and a test fixture, not something you can run in a consumer test.
