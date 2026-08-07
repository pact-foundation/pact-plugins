# JWT plugin (written in Lua)

This is an example plugin supporting creating and matching signed JSON Web Tokens (JWTs) as request/response
bodies with content type `application/jwt` or `application/jwt+json`. Unlike the [CSV](../csv) and
[Protobuf](../protobuf) plugins, it is written in Lua and runs embedded directly in the driver's own process
instead of as a separate compiled gRPC server - see [Writing plugins in Lua](../../docs/writing-plugin-guide.md#writing-plugins-in-lua)
for how that works and what it means for a plugin author.

Only the `RS512` (RSASSA-PKCS1-v1.5 using SHA-512) signing algorithm is supported.

## Files

- [`plugin.lua`](plugin.lua) - the entry point (see `entryPoint` in [`pact-plugin.json`](pact-plugin.json)).
  Defines `init`, `configure_interaction`, and `match_contents`.
- [`jwt.lua`](jwt.lua) - builds/signs/decodes the JWT itself (header, payload, signature).
- [`matching.lua`](matching.lua) - compares an actual token's header/claims/signature/expiry against the expected
  one.
- [`utils.lua`](utils.lua) - small random string/hex helpers used when generating claims the consumer test didn't
  specify explicitly (`sub`, `iss`, `aud`, `jti`).
- [`base64.lua`](base64.lua), [`json.lua`](json.lua), [`inspect.lua`](inspect.lua) - vendored third-party pure-Lua
  libraries (public domain / MIT licensed - see each file's header), used for base64 encoding, JSON encode/decode,
  and pretty-printing debug output respectively. Unrelated to JWT specifically.

## Installing the plugin

There's no build step - it's plain Lua files. Run [`install-local.sh`](install-local.sh), which copies the plugin
directory into `$PACT_PLUGIN_DIR/jwt-0.0.0/` (or `~/.pact/plugins/jwt-0.0.0/` if `PACT_PLUGIN_DIR` isn't set):

```console
$ ./install-local.sh
Installed the jwt plugin into /home/you/.pact/plugins/jwt-0.0.0
```

## Example projects

There are three example projects in [examples/jwt](../../examples/jwt) that use this plugin:

* `jwt-consumer-rust` - consumer written in Rust
* `jwt-consumer-jvm` - consumer written in Java (JUnit 5)
* `jwt-provider-rust` - provider written in Rust

See [examples/jwt/README.md](../../examples/jwt/README.md) for how to run them.

## Configuring an interaction

The consumer test supplies the data to sign the token from, as the interaction's request/response contents (in
pseudo config):

```javascript
"request.contents": {
  "pact:content-type": "application/jwt+json",
  "audience": "1234566778",     // "aud" claim
  "subject": "slksjkdjkdks",    // "sub" claim
  "issuer": "ldsdkdalds",       // "iss" claim
  "algorithm": "RS512",         // only RS512 is supported
  "key-id": "key-112345564",    // "kid" header, optional
  "private-key": "-----BEGIN RSA PRIVATE KEY-----\n...",  // PKCS#1 PEM, required
  "customer_id": "CUST-123456"  // any other field becomes a claim of that name
}
```

Any field that isn't one of the plugin's own (`private-key`, `public-key`, `algorithm`, `key-id`, `subject`,
`issuer`, `audience`) becomes a claim of that name, so `"customer_id": "CUST-123456"` above adds a `customer_id`
claim carrying that value.

`private-key` is the only required field - the rest (`audience`/`subject`/`issuer`/`key-id`) default to random
values if omitted, and `algorithm` defaults to `RS512` (the only one currently supported). A `public-key` field is
also accepted if you'd rather supply it directly than have the plugin derive it from the private key.

The plugin signs a fresh token from this data (with a 5 minute expiry, plus standard `jti`/`iat` claims), and
persists only the **public** key and algorithm into the Pact file - never the private key. Verification only ever
needs to validate a signature, never mint one.

## Matching an interaction

When verifying an actual token against the expected one, the plugin checks, in order:

1. **Signature** - the actual token's signature must validate against the persisted public key.
2. **Expiry** - the actual token's `exp` claim must not be in the past (and `nbf`, if present, must not be in the
   future).
3. **Header** - `typ` and `alg` are compulsory and compared exactly; only registered JWT header names
   (`alg`, `jku`, `jwk`, `kid`, `x5u`, `x5c`, `x5t`, `x5t#S256`, `typ`, `cty`, `crit`) are allowed; `jku` is
   ignored (present or not, its value isn't compared).
4. **Claims** - `iss`, `sub`, `aud` are compulsory and compared exactly; any other claims are allowed and compared
   exactly too, except `exp`/`nbf`/`iat`/`jti` which are always ignored (since they're timestamps/random values
   that legitimately differ between the expected and actual token), and any claim given a matching rule (below).

## Matching rules on a claim

A claim whose value is not fixed between the consumer and the provider can be given a matching rule instead of an
exact value, using the standard integration-JSON form:

```javascript
"request.contents": {
  "pact:content-type": "application/jwt+json",
  "private-key": "-----BEGIN RSA PRIVATE KEY-----\n...",
  "customer_id": {
    "pact:matcher:type": "regex",
    "regex": "CUST-\\d{6}",
    "value": "CUST-123456"
  }
}
```

`value` is the example that goes into the signed token; the rule is what the actual token's claim is checked
against. Any rule name the host Pact framework provides works (`regex`, `type`, `date`, `not-empty`, …), as does
any rule another loaded plugin provides - **this plugin implements none of them itself**. When it meets a claim
with a rule it calls `host_match_field(rule_type, ...)`, which hands the two values to whoever owns that rule (see
proposals [006](../../docs/proposals/006_Field_level_matchers_and_generators.md) and
[009](../../docs/proposals/009_Host_provided_core_matching_and_generation.md)). The mismatch text you see on a
failure is the host's own.

A rule beats the ignore list: `exp` and `iat` are normally skipped, but a test that names one deliberately gets
the rule applied to it.

Rules go into the interaction's ordinary matching rules, keyed by a path into the claims, so the Pact file gets
the standard block:

```json
"matchingRules": {
  "body": {
    "$.claims.customer_id": { "combine": "AND", "matchers": [ { "match": "regex", "regex": "CUST-\\d{6}" } ] }
  }
}
```

The framework stores and returns those paths without interpreting them - it can not traverse a signed token - but
they are the interaction's rules, not this plugin's private configuration, and they are visible to anyone reading
the Pact file. The plugin gets them back in the `rules` field of every match request, on both the consumer and the
provider side. Only the first rule for a path is applied: a claim is a single value, so an `AND` of several rules
has nothing to act on.

The standard rules only reach the plugin if the host framework registers them as core catalogue capabilities,
which needs `pact_matching` 2.0.11+ / Pact-JVM 4.7.5+. Against an older host the plugin reports the claim as a
mismatch explaining that the rule could not be resolved, rather than silently ignoring it.

## Compatibility

Requires a driver with Lua plugin support - as of this writing, that isn't in a released version yet; you'll need
`pact-plugin-driver` and `io.pact.plugin.driver:core` built from this repository's `lua-plugins` branch (Rust: the
default-enabled `lua` cargo feature; JVM: build and `publishToMavenLocal`). See
[examples/jwt/README.md](../../examples/jwt/README.md) for the exact steps, including how to verify the provider
against a locally-built `pact_verifier_cli` in the meantime, since no released provider-verification CLI/library
includes Lua plugin support yet either.
