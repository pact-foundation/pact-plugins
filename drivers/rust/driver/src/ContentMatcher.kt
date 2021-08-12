package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.generators.Generators
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import mu.KLogging

interface ContentMatcher {
  val isCore: Boolean
  val catalogueEntryKey: String
  val pluginName: String

  fun configureContent(
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Triple<OptionalBody, MatchingRuleCategory?, Generators?>
}

data class CatalogueContentMatcher(
  val catalogueEntry: CatalogueEntry
): ContentMatcher {
  override val isCore: Boolean
    get() = catalogueEntry.providerType == CatalogueEntryProviderType.CORE
  override val catalogueEntryKey: String
    get() = "plugin/${catalogueEntry.pluginName}/content-matcher/${catalogueEntry.key}"
  override val pluginName: String
    get() = catalogueEntry.pluginName

  override fun configureContent(
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Triple<OptionalBody, MatchingRuleCategory?, Generators?> {
    logger.debug { "Sending configureContentMatcherInteraction request to for plugin $catalogueEntry" }
    return DefaultPluginManager.configureContentMatcherInteraction(this, contentType, bodyConfig)
  }

  companion object : KLogging()
}
