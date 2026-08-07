package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
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
 * A host-provided handler for the `MatchField` capability shape - one of the standard Pact matching
 * rules applied to a single value. Implemented by the embedding Pact framework and registered via
 * [CoreCapabilityRegistry.registerFieldMatcher]. See proposals 006 (Field-level matchers and
 * generators) and 009 (Host-provided core matching and generation).
 *
 * The request and response are the V2 interface types: field-level operations were introduced in V2
 * and have no V1 equivalent.
 */
fun interface CoreFieldMatcher {
  fun matchField(request: PluginV2.MatchFieldRequest): PluginV2.MatchFieldResponse
}

/**
 * A host-provided handler for the `GenerateField` capability shape - one of the standard Pact
 * generators applied to a single value. Implemented by the embedding Pact framework and registered
 * via [CoreCapabilityRegistry.registerFieldGenerator]. See [CoreFieldMatcher].
 */
fun interface CoreFieldGenerator {
  fun generateField(request: PluginV2.GenerateFieldRequest): PluginV2.GenerateFieldResponse
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
  private val fieldMatchers = ConcurrentHashMap<String, CoreFieldMatcher>()
  private val fieldGenerators = ConcurrentHashMap<String, CoreFieldGenerator>()

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

  /**
   * Register a handler for a host-provided field matching rule, keyed by the catalogue entry key
   * (e.g. `"type"` for the `core/matcher/type` entry). Replaces any handler previously
   * registered under the same key.
   */
  fun registerFieldMatcher(key: String, handler: CoreFieldMatcher) {
    fieldMatchers[key] = handler
  }

  /**
   * Register a handler for a host-provided field generator, keyed by the catalogue entry key
   * (e.g. `"date"` for the `core/generator/date` entry). Replaces any handler previously
   * registered under the same key.
   */
  fun registerFieldGenerator(key: String, handler: CoreFieldGenerator) {
    fieldGenerators[key] = handler
  }

  /**
   * Look up a registered core field matcher handler by catalogue entry key.
   */
  fun fieldMatcher(key: String): CoreFieldMatcher? = fieldMatchers[key]

  /**
   * Look up a registered core field generator handler by catalogue entry key.
   */
  fun fieldGenerator(key: String): CoreFieldGenerator? = fieldGenerators[key]

  /**
   * Remove a registered core field matcher handler. Mainly useful for tests.
   */
  fun deregisterFieldMatcher(key: String) {
    fieldMatchers.remove(key)
  }

  /**
   * Remove a registered core field generator handler. Mainly useful for tests.
   */
  fun deregisterFieldGenerator(key: String) {
    fieldGenerators.remove(key)
  }
}
