package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import java.util.concurrent.ConcurrentHashMap

/**
 * A host-provided handler for the `CompareContents` capability shape. Implemented by the
 * embedding Pact framework and registered via [CoreCapabilityRegistry.registerContentMatcher].
 */
fun interface CoreContentMatcher {
  fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse
}

/**
 * A host-provided handler for the `GenerateContent` capability shape. Implemented by the
 * embedding Pact framework and registered via [CoreCapabilityRegistry.registerContentGenerator].
 */
fun interface CoreContentGenerator {
  fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse
}

/**
 * Registry of host-provided ("core") capability handlers, keyed by catalogue entry key.
 *
 * This mirrors [PluginHostServer]'s instance registry, generalised from a single lookup to one
 * handler per capability shape: the driver defines a narrow interface per capability (matching
 * an operation already defined for plugins), the embedding Pact framework implements it and
 * registers an instance here at startup, and the driver never has a compile-time dependency on
 * that implementation. See proposal 007 (Driver-plugin callback model) for the full design.
 *
 * Registration should happen alongside [CatalogueManager.registerCoreEntries] for the
 * corresponding `CatalogueEntryProviderType.CORE` entry, so an entry and its handler never drift
 * apart. Callers resolve a capability via the catalogue entry's `key` (unprefixed, e.g. `"xml"`
 * for `core/content-matcher/xml`), not the full catalogue key.
 */
object CoreCapabilityRegistry {
  private val contentMatchers = ConcurrentHashMap<String, CoreContentMatcher>()
  private val contentGenerators = ConcurrentHashMap<String, CoreContentGenerator>()

  /**
   * Register a handler for a host-provided content matcher capability, keyed by the catalogue
   * entry key (e.g. `"xml"` for the `core/content-matcher/xml` entry). Replaces any handler
   * previously registered under the same key.
   */
  fun registerContentMatcher(key: String, handler: CoreContentMatcher) {
    contentMatchers[key] = handler
  }

  /**
   * Register a handler for a host-provided content generator capability, keyed by the catalogue
   * entry key (e.g. `"xml"` for the `core/content-generator/xml` entry). Replaces any handler
   * previously registered under the same key.
   */
  fun registerContentGenerator(key: String, handler: CoreContentGenerator) {
    contentGenerators[key] = handler
  }

  /**
   * Look up a registered core content matcher handler by catalogue entry key.
   */
  fun contentMatcher(key: String): CoreContentMatcher? = contentMatchers[key]

  /**
   * Look up a registered core content generator handler by catalogue entry key.
   */
  fun contentGenerator(key: String): CoreContentGenerator? = contentGenerators[key]

  /**
   * Remove a registered core content matcher handler. Mainly useful for tests.
   */
  fun deregisterContentMatcher(key: String) {
    contentMatchers.remove(key)
  }

  /**
   * Remove a registered core content generator handler. Mainly useful for tests.
   */
  fun deregisterContentGenerator(key: String) {
    contentGenerators.remove(key)
  }
}
