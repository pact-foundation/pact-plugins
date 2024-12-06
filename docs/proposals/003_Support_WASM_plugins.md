# Support WASM plugins (Draft)

Discussion for this proposal: https://github.com/pact-foundation/pact-plugins/discussions/85

## Summary

Allow plugins to be written in a language that can compile to WASM + WASI using WIT and the Web Assembly Component model. 

## Definitions

* WASM is the Web Assembly format
* WASI is the Web Assembly System Interface. 
* Web Assembly Component model is a model where WASM files are distributed a components with a well defined interface.
* WIT is the Web Assembly interface definition language (IDL). It forms the interface part of the Web Assembly Component model. 

## Motivation

Web Assembly is designed as a general purpose virtual machine that can execute WASM files (basically a more general 
VM than the JVM). With WASI, it can run server side (WASI brings to Web Assembly what Node brought to the JavaScript world). 
This means that plugins compiled to WASM + WASI are executables that are not dependent on system architectures. Like 
with the JVM, it allows the "write once, run anywhere" paradigm (or the more flippant "Write once, debug everyone" one).
This is very appealing for plugins.

The Web Assembly Component model allows WASM components to be made up of a number components that have a well defined
interface defined by the WIT format (which is very similar to the Protobuf definition). The components can be loaded,
and their interfaces queried and invoked. This is even more appealing for plugins. 

## Details

The WASM based plugins are executed by an embedded interpreter. This means they are run in the same address space as the
testing framework (unlike the gRPC based plugins). Wasmtime was used for the prototype, as it is very standard compliant
and is actively developed. It supports JIT compilation of the executed WASM.

Instead of making RPC calls to the plugin process, the plugin just exposes functions defined by an interface definition 
that the plugin driver can call. These can map quite easily to the gRPC calls make to the exiting plugins (in fact the
WIT format is very similar to the Protobuf one).

For example, with the Init call to the plugin, the plugin returns with the catalogue entries that it supports. With gRPC,
this was defined as

```protobuf
// Entry to be added to the core catalogue. Each entry describes one of the features the plugin provides.
// Entries will be stored in the catalogue under the key "plugin/$name/$type/$key".
message CatalogueEntry {
  enum EntryType {
    // Matcher for contents of messages, requests or response bodies
    CONTENT_MATCHER = 0;
    // Generator for contents of messages, requests or response bodies
    CONTENT_GENERATOR = 1;
    // Transport for a network protocol
    TRANSPORT = 2;
    // Matching rule for content field/values
    MATCHER = 3;
    // Type of interaction
    INTERACTION = 4;
  }
  // Entry type
  EntryType type = 1;
  // Entry key
  string key = 2;
  // Associated data required for the entry. For CONTENT_MATCHER and CONTENT_GENERATOR types, a "content-types"
  // value (separated by semi-colons) is required for all the content types the plugin supports.
  map<string, string> values = 3;
}
```

This can be represented easily with WIT:
```wit
enum entry-type {
    // Matcher for contents of messages, requests or response bodies
    CONTENT-MATCHER,
    // Generator for contents of messages, requests or response bodies
    CONTENT-GENERATOR,
    // Transport for a network protocol
    TRANSPORT,
    // Matching rule for content field/values
    MATCHER,
    // Type of interaction
    INTERACTION
}

// Entry to be added to the core catalogue. Each entry describes one of the features the plugin provides.
// Entries will be stored in the catalogue under the key "plugin/$name/$type/$key".
record catalogue-entry {
  // Entry type
  entry-type: entry-type,
  // Entry key
  key: string,
  // Associated data required for the entry. For CONTENT_MATCHER and CONTENT_GENERATOR types, a "content-types"
  // value (separated by semi-colons) is required for all the content types the plugin supports.
  values: list<tuple<string, string>>
}
```

## Technical details

For a POC of a JWT plugin written in Rust and compiled to WASM, see the feat/wasm-plugins branch in this repository:
* Consumer test https://github.com/pact-foundation/pact-plugins/blob/feat/wasm-plugins/examples/jwt/consumer/src/lib.rs
* JWT Plugin https://github.com/pact-foundation/pact-plugins/tree/feat/wasm-plugins/plugins/jwt/wasm-plugin

All the [gRPC calls and messages](https://github.com/pact-foundation/pact-plugins/blob/feat/wasm-plugins/proto/plugin.proto)
are defined using [WIT instead](https://github.com/pact-foundation/pact-plugins/blob/feat/wasm-plugins/plugins/jwt/wasm-plugin/wit/plugin.wit).

## Benefits

* The plugins are independent of system architecture. They only need to produce a single WASM file.
* Call back functionality (see the [V2 Plugin Interface proposal](https://github.com/pact-foundation/pact-plugins/blob/main/docs/proposals/001_V2_Plugin_Interface.md#capability-for-plugins-to-use-the-functionality-from-the-calling-framework))
  can be easily implemented as functions that are exposed by the driver to the plugin. With the WASM plugin, [the log](https://github.com/pact-foundation/pact-plugins/blob/feat/wasm-plugins/plugins/jwt/wasm-plugin/wit/plugin.wit#L4)
  function is an example. Exporting functions for a WASM file is part of the design of Web Assembly.
* Plugins can be written in any language that compiles to WASM + WASI and supports WIT (there are quite a lot).
* Plugins can have a well defined interface that is part of the plugin binary (WASM file).

## Issues with this approach

* Web Assembly specifications are still in development. Some, like thread support, have only recently gotten to a usable state.
* WASI is missing lots of functionality. While development for it is moving quite fast, there are still gaps. For instance,
  there is no cryptography support. This made writing a JWT plugin challenging, as part of what it needs to do is validate the signature of the token.
* There is no JVM support. There are frameworks like [Extism](https://extism.org/) that support the JVM, they do this by embedding
  the interpreter shared library. This makes the implementation system architecture dependent. It also feels wrong to have
  a stack-based virtual machine embed another stack-based virtual machine.
* There are multiple implementations. Two main ones are Wasmtime and Wasmer. We would need to pick one to use.
