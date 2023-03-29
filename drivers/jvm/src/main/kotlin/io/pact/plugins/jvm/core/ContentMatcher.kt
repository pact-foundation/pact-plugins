package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import au.com.dius.pact.core.support.Result
import mu.KLogging

data class ContentMismatch(
  val expected: ByteArray?,
  val actual: ByteArray?,
  val mismatch: String,
  val path: String,
  val diff : String? = null,
  val type: String? = null
) {
  @Suppress("CyclomaticComplexMethod")
  override fun equals(other: Any?): Boolean {
    if (this === other) return true
    if (javaClass != other?.javaClass) return false

    other as ContentMismatch

    if (expected != null) {
      if (other.expected == null) return false
      if (!expected.contentEquals(other.expected)) return false
    } else if (other.expected != null) return false
    if (actual != null) {
      if (other.actual == null) return false
      if (!actual.contentEquals(other.actual)) return false
    } else if (other.actual != null) return false
    if (mismatch != other.mismatch) return false
    if (path != other.path) return false
    if (diff != other.diff) return false
    if (type != other.type) return false

    return true
  }

  override fun hashCode(): Int {
    var result = expected?.contentHashCode() ?: 0
    result = 31 * result + (actual?.contentHashCode() ?: 0)
    result = 31 * result + mismatch.hashCode()
    result = 31 * result + path.hashCode()
    result = 31 * result + (diff?.hashCode() ?: 0)
    result = 31 * result + type.hashCode()
    return result
  }
}

interface ContentMatcher {
  val isCore: Boolean
  val catalogueEntryKey: String
  val pluginName: String

  fun configureContent(
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Result<List<InteractionContents>, String>

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
  ): Result<List<InteractionContents>, String> {
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
          ContentMismatch(
            it.expected.toByteArray(),
            it.actual.toByteArray(),
            it.mismatch,
            it.path,
            it.diff,
            it.mismatchType
          )
        }
      }
    }
  }

  companion object : KLogging()
}
