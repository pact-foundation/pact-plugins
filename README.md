# pact-plugins
Architecture to support plugins with Pact

* Pact Specification tracking issue: https://github.com/pact-foundation/pact-specification/issues/83
* Plugin Project Board: https://github.com/pact-foundation/pact-plugins/projects/1

## Background

Pact was created initially to support the rise of RESTful microservices and has grown to be the de-facto API contract testing tool.

One of the strengths of Pact is its specification, allowing anybody to create a new language binding in an interoperable way. Whilst this has been great at unifying compatibility, the sprawl of languages makes it hard to add significant new features/behaviour into the framework quickly (e.g. GraphQL or Protobuf support).

**The "shared core"**

We have attempted to combat this time-to-market problem, by focussing on a shared implementation (the "shared  core") in many of the languages. We initially [bundled Ruby](https://docs.pact.io/wrapper_implementations), because it was convenient, but have been slowly moving to our [Rust core](https://github.com/pact-foundation/pact-reference) which solves many of the challenges that bundling Ruby presented.

It is worth noting that the "shared core" approach has largely been a successful exercise in this regard. There are many data points, but the implementation of [WIP/Pending pacts](http://docs.pact.io/pending) was released (elapsed, not effort) in just a few weeks for the libraries that wrapped Ruby. In most cases, an update of the Ruby "binaries", mapping flags from the language specific API to dispatch to the underlying Ruby process, a README update and a release was all that was required. In many cases, new functionality is still published with an update to the Ruby binary, which has been automated through a script.

**Moving beyond HTTP**

But, the industry has continued to innovate since Pact was created in 2013, and RESTful microservices are only one of the key use cases these days - protocols such as Protobufs and Graphql, transports such as TCP, UDP and HTTP/2 and interaction modes (e.g. streaming or server initiated) are starting to become the norm. Standards such as AsyncAPI and CloudEvent are also starting to emerge.

For example, Pact is still a rather HTTP centric library, and the [mixed success](https://docs.pact.io/roadmap/feature_support) in retrofitting "message support" into all languages shows that extensions outside of this boundary aren't trivial, and in some respects are a second class citizen.

The reason is simple: HTTP doesn't change very often, so once a language has implemented a sensible DSL for it and integrated to the core, it's more a matter of fine tuning things. Adding message pact is a paradigm shift relative to HTTP, and requires a whole new developer experience of authoring tests, integrating to the core and so on, for the language author to consider.

Being able to mix and match `protocol`, `transport` and `interaction mode` would be helpful in expanding the use cases. 

Further, being able to add custom contract testing behaviour for bespoke use cases would be helpful in situations where we can't justify the effort to build into the framework itself (custom protocols in banking such as AS2805 come to mind).

To give some sense of magnitude to the challenge, I put this table together well over a year ago that shows some of the Pact deficiencies across popular microservice deployments. In my consulting career (which not-so-coincidentally also aligns quite closely with my Pact maintainership) I've encountered all of those technologies in one form or another.

![83211994-ced39200-a1a1-11ea-8804-19b633cbb1d6](https://user-images.githubusercontent.com/53900/103729694-1e7e1400-5035-11eb-8d4e-641939791552.png)

The "shared core" approach can only take us so far, and we need another mechanism for extending behaviour outside of the responsibilities of this core. This is where I see a plugin approach working with our "shared core" model.
