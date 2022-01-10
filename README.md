# Pact Plugins
> Architecture to support plugins with Pact

* Pact Specification tracking issue: https://github.com/pact-foundation/pact-specification/issues/83
* Plugin Project Board: https://github.com/pact-foundation/pact-plugins/projects/1

[![Pact Plugin Build (Gradle)](https://github.com/pact-foundation/pact-plugins/actions/workflows/gradle.yml/badge.svg)](https://github.com/pact-foundation/pact-plugins/actions/workflows/gradle.yml)
[![Pact Plugin Build (Rust)](https://github.com/pact-foundation/pact-plugins/actions/workflows/rust.yml/badge.svg)](https://github.com/pact-foundation/pact-plugins/actions/workflows/rust.yml)

## Plugin architecture

The plugins are enabled via a message passing mechanism over GRPC. Each language implements a driver which provides the
mechanism to locate and load plugins, as well as a catalogue of features (like mock servers, matchers and provider
verifiers) and a central message bus to enable communication between the language implementation and the plugins.

### Plugin driver

The plugin driver is responsible for providing access to the plugins from the language implementation (which is where the
pact tests are being executed). 

Main responsibilities:
* The ability to find plugins.
* Load plugins and extract the plugin manifests that describe what the plugin provides.
* Provide a catalogue of features provided by the plugins.
* Provide a messaging bus to facilitate communication between the language implementation and the plugins.
* Manage the plugin lifecycles.

See [Plugin driver design docs](docs/plugin-driver-design.md).

There are two implementations of plugin drivers: [JVM](drivers/jvm) and [Rust](drivers/rust).

### Plugins

Plugins are required to start up a GRPC server when loaded, and respond to messages from the plugin driver. They provide
a manifest that describes the features they provide and the mechanism needed to load them.  

Main responsibilities:
* Have a plugin manifest that describes the plugin and how to load it.
* Start a GRPC server on load and provide the port to the driver that loaded it.
* Provide a catalogue of features the plugin provides when the driver requests it.
* Respond to messages from the driver.

See the [guide to writing a Pact plugin](docs/writing-plugin-guide.md).

#### Plugins that provide protocol implementations (WIP)

Plugins can provide support for new protocols. The main features that the plugin would provide is to be
able to create the protocol payloads and create a mock server that can deal with them.

See [Protocol design docs](docs/protocol-plugin-design.md).

This is not implemented as part of the plugin MVC, and will be added in a later update.

#### Plugins that provide support for different types of content

These plugins provide the ability to match and generate different types of contents which are used
with existing protocol implementations. 

See [Content matcher design docs](docs/content-matcher-design.md).

There are two example prototype plugins that support matching different types of content: [Protobuf](plugins/protobuf) and 
[CSV](plugins/csv).

See [Pactflow Protobuf/gRPC plugin](https://github.com/pactflow/pact-protobuf-plugin) for a Pactflow supported plugin.

#### Plugins that provide matchers/generators (WIP)

Plugins can also provide new matching rules and generators. 

TODO, not implemented as part of the plugin MVC, and will be added in a later update.

## Background

Pact was created initially to support the rise of RESTful microservices and has grown to be the de-facto API contract 
testing tool.

One of the strengths of Pact is its specification, allowing anybody to create a new language binding in an interoperable
way. Whilst this has been great at unifying compatibility, the sprawl of languages makes it hard to add significant new
features/behaviour into the framework quickly (e.g. GraphQL or Protobuf support).

**The "shared core"**

We have attempted to combat this time-to-market problem, by focussing on a shared implementation (the "shared  core")
in many of the languages. We initially [bundled Ruby](https://docs.pact.io/wrapper_implementations), because it was 
convenient, but have been slowly moving to our [Rust core](https://github.com/pact-foundation/pact-reference) which 
solves many of the challenges that bundling Ruby presented.

It is worth noting that the "shared core" approach has largely been a successful exercise in this regard. There are 
many data points, but the implementation of [WIP/Pending pacts](http://docs.pact.io/pending) was released (elapsed, 
not effort) in just a few weeks for the libraries that wrapped Ruby. In most cases, an update of the Ruby "binaries", 
mapping flags from the language specific API to dispatch to the underlying Ruby process, a README update and a release
was all that was required. In many cases, new functionality is still published with an update to the Ruby binary, which
has been automated through a script.

**Moving beyond HTTP**

But, the industry has continued to innovate since Pact was created in 2013, and RESTful microservices are only one of 
the key use cases these days - protocols such as Protobufs and Graphql, transports such as TCP, UDP and HTTP/2 and 
interaction modes (e.g. streaming or server initiated) are starting to become the norm. Standards such as AsyncAPI and 
CloudEvent are also starting to emerge.

For example, Pact is still a rather HTTP centric library, and the [mixed success](https://docs.pact.io/roadmap/feature_support)
in retrofitting "message support" into all languages shows that extensions outside of this boundary aren't trivial, 
and in some respects are a second class citizen.

The reason is simple: HTTP doesn't change very often, so once a language has implemented a sensible DSL for it and 
integrated to the core, it's more a matter of fine tuning things. Adding message pact is a paradigm shift relative to 
HTTP, and requires a whole new developer experience of authoring tests, integrating to the core and so on, for the 
language author to consider.

Being able to mix and match `protocol`, `transport` and `interaction mode` would be helpful in expanding the use cases. 

Further, being able to add custom contract testing behaviour for bespoke use cases would be helpful in situations where 
we can't justify the effort to build into the framework itself (custom protocols in banking such as AS2805 come to mind).

To give some sense of magnitude to the challenge, this table shows some of the Pact deficiencies across popular 
microservice deployments.

![83211994-ced39200-a1a1-11ea-8804-19b633cbb1d6](https://user-images.githubusercontent.com/53900/103729694-1e7e1400-5035-11eb-8d4e-641939791552.png)

The "shared core" approach can only take us so far, and we need another mechanism for extending behaviour outside of 
the responsibilities of this core. This is where I see a plugin approach working with our "shared core" model.
