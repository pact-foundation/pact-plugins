package io.pact.plugins.jvm.core

import io.pact.core.model.ContentType
import io.pact.plugin.Plugin
import mu.KLogging
import java.lang.IllegalArgumentException

object CatalogueManager : KLogging() {
  private val catalogue = mutableMapOf<String, CatalogueEntry>()

  fun registerPluginEntries(name: String, catalogueList: List<Plugin.CatalogueEntry>) {
    catalogueList.forEach {
      val type = CatalogueEntryType.fromString(it.type)
      val key = "plugin/$name/${type}/${it.key}"
      catalogue[key] = CatalogueEntry(type, CatalogueEntryProviderType.PLUGIN, name, it.valuesMap)
    }

    logger.debug { "Updated catalogue entries:\n${catalogue.keys.joinToString("\n")}" }
  }

  fun registerCoreEntries(entries: List<CatalogueEntry>) {
    entries.forEach {
      val key = "core/${it.type}/${it.key}"
      catalogue[key] = it
    }

    logger.debug { "Core catalogue entries:\n${catalogue.keys.joinToString("\n")}" }
  }

  fun entries() = catalogue.entries

  fun findContentMatcher(contentType: ContentType): ContentMatcher? {
    val catalogueEntry = catalogue.values.find { entry ->
      if (entry.type == CatalogueEntryType.CONTENT_MATCHER) {
        val contentTypes = entry.values["content-types"]?.split(';')
        if (contentTypes.isNullOrEmpty()) {
          false
        } else {
          contentTypes.any { contentType.matches(it) }
        }
      } else {
        false
      }
    }
    return if (catalogueEntry != null)
      CatalogueContentMatcher(catalogueEntry)
      else null
  }
}

enum class CatalogueEntryType {
  CONTENT_MATCHER, MOCK_SERVER, MATCHER;

  companion object {
    fun fromString(type: String): CatalogueEntryType {
      return when (type) {
        "content-matcher" -> CONTENT_MATCHER
        "matcher" -> MATCHER
        "mock-server" -> MOCK_SERVER
        else -> throw IllegalArgumentException("'$type' is not a valid CatalogueEntryType value")
      }
    }
  }
}

data class CatalogueEntry(
  val type: CatalogueEntryType,
  val providerType: CatalogueEntryProviderType,
  val key: String,
  val values: Map<String, String> = mapOf()
)

enum class CatalogueEntryProviderType {
  CORE, PLUGIN
}
