package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import io.github.oshai.kotlinlogging.KotlinLogging
import io.pact.plugin.Plugin
import java.lang.IllegalArgumentException

private val logger = KotlinLogging.logger {}

/**
 * The catalogue manager stores all the entries from the core Pact framework as well as all the loaded plugins
 */
object CatalogueManager {
  private val catalogue = mutableMapOf<String, CatalogueEntry>()

  /**
   * Register the list of entries against the plugin name. Each entry will be keyed by
   * plugin/<plugin-name>/<entry-type>/<entry-key>
   */
  fun registerPluginEntries(name: String, catalogueList: List<Plugin.CatalogueEntry>) {
    catalogueList.forEach {
      val type = CatalogueEntryType.fromEntry(it.type)
      val key = "plugin/$name/${type}/${it.key}"
      catalogue[key] = CatalogueEntry(type, CatalogueEntryProviderType.PLUGIN, name, it.key, it.valuesMap)
    }

    logger.debug { "Updated catalogue entries:\n${catalogue.keys.joinToString("\n")}" }
  }

  /**
   * INTERNAL: Register the entries as core Pact framework entries
   */
  fun registerCoreEntries(entries: List<CatalogueEntry>) {
    entries.forEach {
      val key = "core/${it.type}/${it.key}"
      catalogue[key] = it
    }

    logger.debug { "Core catalogue entries:\n${catalogue.keys.joinToString("\n")}" }
  }

  /**
   * Return all the entries from the catalogue
   */
  fun entries() = catalogue.entries

  /**
   * Return entries provided by the core framework (excludes plugin entries)
   */
  fun coreEntries() = catalogue.values.filter { it.providerType == CatalogueEntryProviderType.CORE }

  /**
   * Lookup entry by key. Entries are keyed by <core|plugin>/<plugin-name>?/<entry-type>/<entry-key>
   */
  fun lookupEntry(key: String): CatalogueEntry? {
    return catalogue[key] ?: catalogue.entries.firstOrNull {
      it.key.endsWith(key)
    }?.value
  }

  /**
   * Resolve a callback's catalogue entry key to a dispatch target, the same way
   * [CatalogueContentMatcher.isCore]/[CatalogueContentGenerator.isCore] do for the driver's own
   * outbound calls. Shared by every transport that lets a plugin call back into a capability by
   * catalogue entry key - the gRPC `PluginHost` service ([PluginHostServer]) and the Lua host
   * functions ([LuaPactPlugin]) - so there is exactly one place that decides "who provides this
   * entry". See proposal 007 ("One resolver, multiple call directions").
   *
   * `entryKey` is matched by suffix against the full catalogue key (e.g. a plugin passing `"xml"`
   * matches `"core/content-matcher/xml"`) via [lookupEntry]. That means an unqualified key could
   * coincidentally match an entry of the wrong capability shape (a content-generator registered
   * under the same name as an unrelated content-matcher); `expectedType` guards against silently
   * dispatching to it, mirroring the explicit `type` check [findContentMatcher]/
   * [findContentGenerator] already do.
   */
  fun resolveCapability(entryKey: String, expectedType: CatalogueEntryType): ResolvedCapability {
    val entry = lookupEntry(entryKey) ?: throw PactCatalogueEntryNotFoundException(entryKey)
    if (entry.type != expectedType) {
      throw PactCatalogueEntryTypeMismatchException(entryKey, entry.type, expectedType)
    }
    return when (entry.providerType) {
      CatalogueEntryProviderType.CORE -> ResolvedCapability.Core(entry.key)
      CatalogueEntryProviderType.PLUGIN -> ResolvedCapability.Plugin(entry.pluginName)
    }
  }

  /**
   * Lookup a content matcher in the catalogue that can handle the given content type
   */
  fun findContentMatcher(contentType: ContentType): ContentMatcher? {
    val catalogueEntry = catalogue.values.find { entry ->
      if (entry.type == CatalogueEntryType.CONTENT_MATCHER) {
        val contentTypes = entry.values["content-types"]?.split(';')
        if (contentTypes.isNullOrEmpty()) {
          false
        } else {
          contentTypes.any { contentType.matches(it.trim()) }
        }
      } else {
        false
      }
    }
    return if (catalogueEntry != null)
      CatalogueContentMatcher(catalogueEntry)
      else null
  }

  /**
   * Lookup the content generator the can handle the given content type
   */
  fun findContentGenerator(contentType: ContentType): ContentGenerator? {
    val catalogueEntry = catalogue.values.find { entry ->
      if (entry.type == CatalogueEntryType.CONTENT_GENERATOR) {
        val contentTypes = entry.values["content-types"]?.split(';')
        if (contentTypes.isNullOrEmpty()) {
          false
        } else {
          contentTypes.any { contentType.matches(it.trim()) }
        }
      } else {
        false
      }
    }
    return if (catalogueEntry != null)
      CatalogueContentGenerator(catalogueEntry)
    else null
  }

  /**
   * Remove entries for a plugin
   */
  fun removePluginEntries(name: String) {
    val prefix = "plugin/$name/"
    catalogue.values.removeIf {
      it.key.startsWith(prefix)
    }

    logger.debug { "Removed all catalogue entries for plugin $name" }
  }
}

/**
 * Checks if a registered content-type pattern matches this content type. The pattern is
 * matched as a regex against the base type (i.e. with any parameters like `charset`
 * stripped); Kotlin's `matches` is a full match (the whole base type must match, not just a
 * substring) - this must stay consistent with the equivalent check in the Rust driver's
 * `catalogue_manager::matches_pattern`, so a plugin's catalogue registration behaves the same
 * way regardless of which driver loaded it. Regex metacharacters in a content type (most
 * commonly `+`, as in a `+json`/`+xml` structured syntax suffix) need to be escaped by the
 * plugin author for a literal match.
 */
private fun ContentType.matches(type: String) = this.getBaseType().orEmpty().matches(Regex(type))

/**
 * Type of entry in the catalogue
 */
enum class CatalogueEntryType {
  CONTENT_MATCHER, CONTENT_GENERATOR, TRANSPORT, MATCHER, INTERACTION;

  override fun toString(): String {
    return when (this) {
      CONTENT_MATCHER -> "content-matcher"
      CONTENT_GENERATOR -> "content-generator"
      TRANSPORT -> "transport"
      MATCHER -> "matcher"
      INTERACTION -> "interaction"
    }
  }

  /**
   * Convert this entry type to the matching Protobuf type
   */
  fun toEntry(): Plugin.CatalogueEntry.EntryType {
    return when (this) {
      CONTENT_MATCHER -> Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER
      CONTENT_GENERATOR -> Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR
      TRANSPORT -> Plugin.CatalogueEntry.EntryType.TRANSPORT
      MATCHER -> Plugin.CatalogueEntry.EntryType.MATCHER
      INTERACTION -> Plugin.CatalogueEntry.EntryType.INTERACTION
    }
  }

  companion object {
    /**
     * Return the corresponding entry type from the given string value
     */
    fun fromString(type: String): CatalogueEntryType {
      return when (type) {
        "content-matcher" -> CONTENT_MATCHER
        "content-generator" -> CONTENT_GENERATOR
        "interaction" -> INTERACTION
        "matcher" -> MATCHER
        "transport" -> TRANSPORT
        else -> throw IllegalArgumentException("'$type' is not a valid CatalogueEntryType value")
      }
    }

    /**
     * Return the catalogue entry type from the corresponding Protobuf entry type
     */
    fun fromEntry(type: Plugin.CatalogueEntry.EntryType?): CatalogueEntryType {
      return if (type != null) {
        when (type) {
          Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER -> CONTENT_MATCHER
          Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR -> CONTENT_GENERATOR
          Plugin.CatalogueEntry.EntryType.TRANSPORT -> TRANSPORT
          Plugin.CatalogueEntry.EntryType.MATCHER -> MATCHER
          Plugin.CatalogueEntry.EntryType.INTERACTION -> INTERACTION
          Plugin.CatalogueEntry.EntryType.UNRECOGNIZED -> CONTENT_MATCHER
        }
      } else {
        CONTENT_MATCHER
      }
    }
  }
}

/**
 * Entry in the catalogue
 */
data class CatalogueEntry @JvmOverloads constructor(
  /**
   * Type of entry
   */
  val type: CatalogueEntryType,

  /**
   * What provides the entry (core framework or plugin)
   */
  val providerType: CatalogueEntryProviderType,

  /**
   * Plugin name that provides the entry (may not be set for core entries)
   */
  val pluginName: String,

  /**
   * Key for the entry
   */
  val key: String,

  /**
   * Associated values for the entry
   */
  val values: Map<String, String> = mapOf()
)

/**
 * Type of provider for an entry in the catalogue
 */
enum class CatalogueEntryProviderType {
  CORE, PLUGIN
}

/** Where a resolved catalogue entry's capability should be dispatched. See
 * [CatalogueManager.resolveCapability]. */
sealed class ResolvedCapability {
  /** A host-registered core handler, keyed by the unprefixed catalogue entry key. */
  data class Core(val key: String) : ResolvedCapability()
  /** A running plugin, identified by its name. */
  data class Plugin(val pluginName: String) : ResolvedCapability()
}
