package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import io.github.oshai.kotlinlogging.KotlinLogging
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
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
      // Deliberately reading the raw field rather than the generated `type` accessor: that accessor
      // is generated against the V1 enum and reports anything it doesn't recognise as UNRECOGNIZED,
      // which fromEntry maps to CONTENT_MATCHER - silently registering a V2-only entry type, a
      // GENERATOR say, as a content matcher.
      val type = CatalogueEntryType.fromEntryValue(it.typeValue)
      if (type == null) {
        logger.warn {
          "Ignoring catalogue entry '${it.key}' from plugin '$name': ${it.typeValue} is not a " +
            "catalogue entry type this driver understands"
        }
      } else {
        val key = "plugin/$name/${type}/${it.key}"
        catalogue[key] = CatalogueEntry(type, CatalogueEntryProviderType.PLUGIN, name, it.key, it.valuesMap)
      }
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
   * Lookup entry by key. Entries are keyed by <core|plugin>/<plugin-name>?/<entry-type>/<entry-key>,
   * and are matched the same way [resolveCapability] matches them: by name first - the whole
   * catalogue key, or a trailing run of its `/`-separated components - then against the core
   * catalogue's versioned naming convention.
   *
   * Unlike [resolveCapability], this takes the first match when more than one entry matches. Prefer
   * [resolveCapability] wherever the expected entry type is known and a deterministic answer
   * matters.
   */
  fun lookupEntry(key: String): CatalogueEntry? {
    return catalogue.entries.firstOrNull { namesCatalogueKey(it.key, key) }?.value
      ?: catalogue.entries.firstOrNull { namesVersionedCoreKey(it.value, key) }?.value
  }

  /**
   * Resolve a callback's catalogue entry key to a dispatch target, the same way
   * [CatalogueContentMatcher.isCore]/[CatalogueContentGenerator.isCore] do for the driver's own
   * outbound calls. Shared by every transport that lets a plugin call back into a capability by
   * catalogue entry key - the gRPC `PluginHost` service ([PluginHostServer]) and the Lua host
   * functions ([LuaPactPlugin]) - so there is exactly one place that decides "who provides this
   * entry". See proposal 007 ("One resolver, multiple call directions").
   *
   * `entryKey` is matched against the catalogue in two passes:
   *
   * 1. by name - the whole catalogue key (`core/content-matcher/xml`) or a trailing run of its
   *    components (`content-matcher/xml`, `xml`), compared component by component;
   * 2. failing that, against the core catalogue's versioned naming convention, so `type` resolves
   *    to `v2-type` and `date` to `v3-date`.
   *
   * Naming an entry directly always wins over the versioned fallback: if a plugin registers its
   * own `matcher/date`, `date` resolves to that plugin, and a caller that specifically wants the
   * core rule asks for `v3-date`.
   *
   * Unlike [lookupEntry], this does not just take the first hit when more than one entry matches.
   * A short, unqualified `entryKey` can still match more than one entry of the *same*
   * `expectedType` (a plugin registering its own `content-matcher/xml` alongside a host-registered
   * core `content-matcher/xml`); silently picking one (in whatever order the backing map iterates)
   * would make the dispatch target depend on registration order rather than being deterministic.
   * `expectedType` still guards against the *wrong* capability shape (a content-generator
   * registered under the same name as an unrelated content-matcher), mirroring the explicit `type`
   * check [findContentMatcher]/[findContentGenerator] already do.
   */
  fun resolveCapability(entryKey: String, expectedType: CatalogueEntryType): ResolvedCapability {
    val named = catalogue.entries.filter { namesCatalogueKey(it.key, entryKey) }.map { it.key to it.value }
    val candidates = if (named.any { (_, entry) -> entry.type == expectedType }) {
      named
    } else {
      val versioned = catalogue.entries
        .filter { namesVersionedCoreKey(it.value, entryKey) }
        .map { it.key to it.value }
      // Keep any wrong-typed direct matches for the diagnostic below if the fallback finds nothing
      if (versioned.isEmpty()) named else versioned
    }

    val ofExpectedType = candidates.filter { (_, entry) -> entry.type == expectedType }
    val entry = when (ofExpectedType.size) {
      0 -> when (val wrongType = candidates.firstOrNull()?.second) {
        null -> throw PactCatalogueEntryNotFoundException(entryKey)
        else -> throw PactCatalogueEntryTypeMismatchException(entryKey, wrongType.type, expectedType)
      }
      1 -> ofExpectedType[0].second
      else -> throw PactCatalogueEntryAmbiguousException(entryKey, ofExpectedType.map { it.first }.sorted())
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
 * Does `entryKey` name this catalogue key? Either the whole key (`core/content-matcher/xml`), or a
 * trailing run of its `/`-separated components (`content-matcher/xml`, or just `xml`).
 *
 * Components are compared whole, never as substrings: `"type"` does not name
 * `core/matcher/v2-type`. That distinction is the point - the core catalogue prefixes the Pact
 * specification version a matcher was introduced in to its name, so a substring match would let an
 * unqualified name collide with a versioned core key it has nothing to do with (a plugin's own
 * `matcher/date` against `core/matcher/v3-date`, say). The versioned form is handled deliberately
 * by [namesVersionedCoreKey] instead, as a fallback rather than a coincidence.
 *
 * Must stay consistent with `catalogue_manager::names_catalogue_key` in the Rust driver.
 */
internal fun namesCatalogueKey(catalogueKey: String, entryKey: String): Boolean {
  val keyParts = catalogueKey.split('/')
  val queryParts = entryKey.split('/')
  return queryParts.size <= keyParts.size &&
    keyParts.subList(keyParts.size - queryParts.size, keyParts.size) == queryParts
}

private val VERSION_PREFIX = Regex("^v\\d+$")

/**
 * Does `entryKey` name this entry under the core catalogue's versioned naming convention - the Pact
 * specification version the rule was introduced in, prefixed to its name (`v2-type`, `v3-date`,
 * `v4-not-empty`)? This is what lets a caller ask for `type` and get `v2-type` without having to
 * know which specification version introduced it.
 *
 * Only the whole name after the version prefix counts, so `type` names `v2-type` but not
 * `v3-content-type` or `v2-min-type` - those are the `content-type` and `min-type` rules.
 *
 * The convention only applies to matching rules and generators. Content matchers, content
 * generators and transports are registered under plain names (`xml`, `json`, `grpc`), so a leading
 * `v<n>-` there is part of the name rather than a version, and stripping it would be wrong.
 *
 * Must stay consistent with `catalogue_manager::names_versioned_core_key` in the Rust driver.
 */
internal fun namesVersionedCoreKey(entry: CatalogueEntry, entryKey: String): Boolean {
  if (entry.type != CatalogueEntryType.MATCHER && entry.type != CatalogueEntryType.GENERATOR) {
    return false
  }
  val separator = entry.key.indexOf('-')
  if (separator <= 0) return false
  return entry.key.substring(separator + 1) == entryKey &&
    VERSION_PREFIX.matches(entry.key.substring(0, separator))
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
  CONTENT_MATCHER, CONTENT_GENERATOR, TRANSPORT, MATCHER, INTERACTION,

  /**
   * Generator for a content field/value. Only representable on the V2 plugin interface - the V1
   * EntryType enum has no equivalent. See proposal 006 (Field-level matchers and generators).
   */
  GENERATOR;

  override fun toString(): String {
    return when (this) {
      CONTENT_MATCHER -> "content-matcher"
      CONTENT_GENERATOR -> "content-generator"
      TRANSPORT -> "transport"
      MATCHER -> "matcher"
      INTERACTION -> "interaction"
      GENERATOR -> "generator"
    }
  }

  /**
   * Convert this entry type to the matching Protobuf type.
   *
   * This maps to the *V1* enum, which cannot represent every entry type: GENERATOR has no V1
   * equivalent and is reported as MATCHER. Use [toEntryValue] instead, which returns the wire
   * value and is lossless for both interface versions.
   */
  @Deprecated(
    message = "Use toEntryValue, which can represent entry types that only exist on the V2 interface",
    replaceWith = ReplaceWith("toEntryValue()")
  )
  fun toEntry(): Plugin.CatalogueEntry.EntryType {
    return when (this) {
      CONTENT_MATCHER -> Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER
      CONTENT_GENERATOR -> Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR
      TRANSPORT -> Plugin.CatalogueEntry.EntryType.TRANSPORT
      MATCHER -> Plugin.CatalogueEntry.EntryType.MATCHER
      INTERACTION -> Plugin.CatalogueEntry.EntryType.INTERACTION
      GENERATOR -> Plugin.CatalogueEntry.EntryType.MATCHER
    }
  }

  /**
   * The V2 Protobuf enum for this entry type. V2 is the canonical source of the wire values: it
   * mirrors V1 for the entry types both versions share, and adds the ones V1 never had.
   */
  private fun toEntryV2(): PluginV2.CatalogueEntry.EntryType {
    return when (this) {
      CONTENT_MATCHER -> PluginV2.CatalogueEntry.EntryType.CONTENT_MATCHER
      CONTENT_GENERATOR -> PluginV2.CatalogueEntry.EntryType.CONTENT_GENERATOR
      TRANSPORT -> PluginV2.CatalogueEntry.EntryType.TRANSPORT
      MATCHER -> PluginV2.CatalogueEntry.EntryType.MATCHER
      INTERACTION -> PluginV2.CatalogueEntry.EntryType.INTERACTION
      GENERATOR -> PluginV2.CatalogueEntry.EntryType.GENERATOR
    }
  }

  /**
   * The Protobuf enum value for this entry type, for setting the `type` field of a CatalogueEntry
   * message with `setTypeValue`. Lossless for every entry type, including those a V1 plugin will
   * not recognise - a V1 plugin decodes an unknown value as UNRECOGNIZED and ignores the entry,
   * which is the correct outcome, whereas mapping it onto some other V1 type would have the plugin
   * act on an entry that is not what it thinks it is.
   */
  fun toEntryValue(): Int = toEntryV2().number

  /**
   * The Protobuf enum value name for this entry type, e.g. "CONTENT_MATCHER". This is the form a
   * Lua plugin uses in the catalogue entries returned from its `init` function.
   */
  fun toEntryName(): String = toEntryV2().name

  companion object {
    /**
     * Return the corresponding entry type from the given string value
     */
    @JvmStatic
    fun fromString(type: String): CatalogueEntryType {
      return when (type) {
        "content-matcher" -> CONTENT_MATCHER
        "content-generator" -> CONTENT_GENERATOR
        "interaction" -> INTERACTION
        "matcher" -> MATCHER
        "transport" -> TRANSPORT
        "generator" -> GENERATOR
        else -> throw IllegalArgumentException("'$type' is not a valid CatalogueEntryType value")
      }
    }

    /**
     * Return the catalogue entry type from the corresponding Protobuf entry type
     */
    @Deprecated(
      message = "Use fromEntryValue, which can represent entry types that only exist on the V2 interface",
      replaceWith = ReplaceWith("fromEntryValue(type?.number ?: 0)")
    )
    @JvmStatic
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

    /**
     * The entry type for a Protobuf enum value, or null if the value is not one this driver
     * understands (an entry type added by a later interface version). Deliberately not defaulting
     * to CONTENT_MATCHER the way the generated V1 accessor does: silently mis-typing an entry is
     * worse than ignoring one.
     */
    @JvmStatic
    fun fromEntryValue(value: Int): CatalogueEntryType? {
      return when (PluginV2.CatalogueEntry.EntryType.forNumber(value)) {
        PluginV2.CatalogueEntry.EntryType.CONTENT_MATCHER -> CONTENT_MATCHER
        PluginV2.CatalogueEntry.EntryType.CONTENT_GENERATOR -> CONTENT_GENERATOR
        PluginV2.CatalogueEntry.EntryType.TRANSPORT -> TRANSPORT
        PluginV2.CatalogueEntry.EntryType.MATCHER -> MATCHER
        PluginV2.CatalogueEntry.EntryType.INTERACTION -> INTERACTION
        PluginV2.CatalogueEntry.EntryType.GENERATOR -> GENERATOR
        else -> null
      }
    }

    /**
     * The entry type for a Protobuf enum value name, e.g. "CONTENT_MATCHER", or null if the name is
     * not one this driver understands.
     */
    @JvmStatic
    fun fromEntryName(name: String): CatalogueEntryType? {
      val entryType = PluginV2.CatalogueEntry.EntryType.values().find { it.name == name }
      return if (entryType != null) fromEntryValue(entryType.number) else null
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
