package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.generators.Generator
import mu.KLogging

/**
 * Interface to a content generator
 */
interface ContentGenerator {
  val catalogueEntry: CatalogueEntry
  /**
   * If this is a core generator or from a plugin
   */
  val isCore: Boolean

  /**
   * Generate the contents for the body, using the provided generators
   */
  fun generateContent(contentType: ContentType, generators: Map<String, Generator>, body: OptionalBody): OptionalBody
}

open class CatalogueContentGenerator(override val catalogueEntry: CatalogueEntry) : ContentGenerator, KLogging() {
  override val isCore: Boolean
    get() = catalogueEntry.providerType == CatalogueEntryProviderType.CORE

  override fun generateContent(
    contentType: ContentType,
    generators: Map<String, Generator>,
    body: OptionalBody
  ): OptionalBody {
    return DefaultPluginManager.generateContent(this, contentType, generators, body)
  }
}
