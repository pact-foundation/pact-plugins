# JSON-RPC plugin

This is a reference Pact transport plugin for **JSON-RPC 2.0 over HTTP**.

It is intentionally small and focused on the transport lifecycle:

- configuring an RPC interaction from a consumer test
- starting a mock server for those interactions
- preparing an interaction for provider verification
- verifying a provider by making a JSON-RPC request over HTTP

The first cut keeps matching deliberately simple and uses exact example values from the Pact interaction configuration.

## Consumer interaction shape

The plugin expects interaction configuration in this form:

```json
{
  "pact:content-type": "application/json-rpc",
  "path": "/rpc",
  "request": {
    "method": "sum",
    "params": [2, 3],
    "id": 1
  },
  "response": {
    "result": 5
  }
}
```

The plugin persists this configuration in the Pact file and uses it for both the mock server and provider verification.

## Local development

Build the plugin:

```sh
cargo build
```

Install it into the local Pact plugin directory for examples:

```sh
mkdir -p ~/.pact/plugins/jsonrpc-0.1.0-beta.1
cp pact-plugin.json ~/.pact/plugins/jsonrpc-0.1.0-beta.1/
cp target/debug/pact-jsonrpc-plugin ~/.pact/plugins/jsonrpc-0.1.0-beta.1/
```
