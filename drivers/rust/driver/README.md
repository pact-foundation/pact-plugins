# Pact plugin driver library for Rust
> Pact support library that provides an interface for interacting with Pact plugins

## State of implementation

* [X] The ability to find plugins.
* [X] Load plugins and extract the plugin manifests that describe what the plugin provides.
* [X] Provide a catalogue of features provided by the plugins.
* [ ] Provide a messaging bus to facilitate communication between the language implementation and the plugins.
* [X] Manage the plugin lifecycles.

## Crate features

All features are enabled by default

* `datetime`: Enables support of date and time expressions and generators. This will add the
  `chronos` crate as a dependency.
* `xml`: Enables support for parsing XML documents. This feature will add the `sxd-document`
  crate as a dependency.
* `lua`: Enables plugins written in Lua (Lua 5.4).

## Building the Rust driver

The Rust driver is built with Cargo. The build can be run with `cargo build`, but there is a test `DriverPactTest`
that requires a Protobuf plugin to work. Either skip or disable that test, or install the prototype Protobuf plugin before
running the build.

The Protobuf compiler is required to be installed and available on the path.
