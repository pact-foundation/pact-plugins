# Content Matchers and Generators

Plugins can provide matchers and generators for different types of content. These contents are the bodies of 
requests and responses and payloads of messages. Matchers are able to compare the contents against the ones from
the Pact interactions, and generators create the contents for use in tests.

If a plugin provides a content matcher, they should also provide a generator.

We will use the [CSV plugin](../plugins/csv) as an example of a plugin that provides a matcher and generator.

