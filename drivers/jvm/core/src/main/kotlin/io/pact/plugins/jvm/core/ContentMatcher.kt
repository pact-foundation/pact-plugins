package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.generators.Generators
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import com.github.michaelbull.result.Result
import mu.KLogging

data class ContentMismatch(
  val expected: ByteArray?,
  val actual: ByteArray?,
  val mismatch: String,
  val path: String,
  val diff : String? = null
)

interface ContentMatcher {
  val isCore: Boolean
  val catalogueEntryKey: String
  val pluginName: String

  fun configureContent(
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Result<InteractionContents, String>

  fun invokeContentMatcher(
    expected: OptionalBody,
    actual: OptionalBody,
    allowUnexpectedKeys: Boolean,
    rules: Map<String, MatchingRuleGroup>,
    pluginConfiguration: Map<String, PluginConfiguration>
  ): Map<String, List<ContentMismatch>>
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
  ): Result<InteractionContents, String> {
    logger.debug { "Sending configureContentMatcherInteraction request to for plugin $catalogueEntry" }
    return DefaultPluginManager.configureContentMatcherInteraction(this, contentType, bodyConfig)
  }

  override fun invokeContentMatcher(
    expected: OptionalBody,
    actual: OptionalBody,
    allowUnexpectedKeys: Boolean,
    rules: Map<String, MatchingRuleGroup>,
    pluginConfiguration: Map<String, PluginConfiguration>
  ): Map<String, List<ContentMismatch>> {
    logger.debug { "invokeContentMatcher(allowUnexpectedKeys=$allowUnexpectedKeys, rules=$rules, " +
      "pluginConfiguration=$pluginConfiguration)" }
    val result = DefaultPluginManager.invokeContentMatcher(this, expected, actual, allowUnexpectedKeys, rules,
      pluginConfiguration)
    return if (result.error.isNotEmpty()) {
      mapOf("$" to listOf(ContentMismatch(expected.value, actual.value, result.error, "$")))
    } else {
      result.resultsMap.mapValues { entry ->
        entry.value.mismatchesList.map {
          ContentMismatch(it.expected.toByteArray(), it.actual.toByteArray(), it.mismatch, it.path, it.diff)
        }
      }
    }
  }

  companion object : KLogging()
}
