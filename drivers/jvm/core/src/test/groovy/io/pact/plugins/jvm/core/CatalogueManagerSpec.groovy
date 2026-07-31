package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import spock.lang.Specification

class CatalogueManagerSpec extends Specification {
  def 'sets plugin catalogue entries correctly'() {
    given:
    def matcherEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER)
      .setKey('protobuf-test')
      .putValues('content-types', 'application/protobuf-test;application/grpc-test')
      .build()
    def generatorEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR)
      .setKey('protobuf-test')
      .putValues('content-types', 'application/protobuf-test;application/grpc-test')
      .build()
    def transportEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.TRANSPORT)
      .setKey('grpc-test')
      .build()
    def entries = [
      matcherEntry,
      generatorEntry,
      transportEntry
    ]

    when:
    CatalogueManager.INSTANCE.registerPluginEntries("CatalogueManagerSpec", entries)
    def contentMatcher = CatalogueManager.INSTANCE.lookupEntry('content-matcher/protobuf-test')
    def contentGenerator = CatalogueManager.INSTANCE.lookupEntry('content-generator/protobuf-test')
    def transport = CatalogueManager.INSTANCE.lookupEntry('transport/grpc-test')

    then:
    contentMatcher == new CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec', 'protobuf-test', ['content-types': 'application/protobuf-test;application/grpc-test'])
    contentGenerator == new CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec', 'protobuf-test', ['content-types': 'application/protobuf-test;application/grpc-test'])
    transport == new CatalogueEntry(CatalogueEntryType.TRANSPORT, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec', 'grpc-test')

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec')
  }

  def 'entry type protobuf values and names round trip'() {
    expect:
    CatalogueEntryType.fromEntryValue(type.toEntryValue()) == type
    CatalogueEntryType.fromEntryName(type.toEntryName()) == type
    // The toString form round-trips too - it is what catalogue keys are built from
    CatalogueEntryType.fromString(type.toString()) == type

    where:
    type << CatalogueEntryType.values()
  }

  def 'entry type conversions return null for values this driver does not understand'() {
    expect:
    CatalogueEntryType.fromEntryValue(99) == null
    CatalogueEntryType.fromEntryName('NOT_AN_ENTRY_TYPE') == null
  }

  def 'entry types shared with V1 keep their V1 wire values'() {
    // The wire values come from the V2 enum, but the driver publishes one catalogue to every
    // running plugin, V1 ones included - so the values V1 knows about must not shift.
    expect:
    CatalogueEntryType.CONTENT_MATCHER.toEntryValue() == Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER.number
    CatalogueEntryType.CONTENT_GENERATOR.toEntryValue() == Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR.number
    CatalogueEntryType.TRANSPORT.toEntryValue() == Plugin.CatalogueEntry.EntryType.TRANSPORT.number
    CatalogueEntryType.MATCHER.toEntryValue() == Plugin.CatalogueEntry.EntryType.MATCHER.number
    CatalogueEntryType.INTERACTION.toEntryValue() == Plugin.CatalogueEntry.EntryType.INTERACTION.number
  }

  def 'registers a generator entry under its own entry type'() {
    given:
    // GENERATOR only exists on the V2 enum. This is exactly the case where the generated V1 `type`
    // accessor would report UNRECOGNIZED, which fromEntry maps to CONTENT_MATCHER.
    def generatorEntry = Plugin.CatalogueEntry.newBuilder()
      .setTypeValue(CatalogueEntryType.GENERATOR.toEntryValue())
      .setKey('creditcard-test')
      .putValues('config-key', 'brand')
      .build()

    when:
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-generator', [generatorEntry])
    def generator = CatalogueManager.INSTANCE.lookupEntry('generator/creditcard-test')
    def asContentMatcher = CatalogueManager.INSTANCE.lookupEntry('content-matcher/creditcard-test')

    then:
    generator == new CatalogueEntry(CatalogueEntryType.GENERATOR, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec-generator', 'creditcard-test', ['config-key': 'brand'])
    asContentMatcher == null

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-generator')
  }

  def 'ignores a catalogue entry whose type this driver does not understand'() {
    given:
    def entry = Plugin.CatalogueEntry.newBuilder()
      .setTypeValue(99)
      .setKey('unknown-entry-type-test')
      .build()

    when:
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-unknown', [entry])

    then:
    CatalogueManager.INSTANCE.entries().find { it.value.key == 'unknown-entry-type-test' } == null

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-unknown')
  }

  def 'resolveCapability resolves an unambiguous core entry'() {
    given:
    def key = 'resolveCapability resolves an unambiguous core entry'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.CORE, '', key)
    ])

    when:
    def resolved = CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.CONTENT_MATCHER)

    then:
    resolved instanceof ResolvedCapability.Core
    ((ResolvedCapability.Core) resolved).key == key
  }

  /**
   * The core matcher entries as the Pact frameworks actually register them - the Pact
   * specification version the rule was introduced in, prefixed to the name. Kept in sync with
   * MatcherExecutor.kt in Pact-JVM and MATCHER_CATALOGUE_ENTRIES in pact_matching.
   */
  private static void registerCoreMatcherEntries() {
    CatalogueManager.INSTANCE.registerCoreEntries(
      ['v2-regex', 'v2-type', 'v3-number-type', 'v3-integer-type', 'v3-decimal-type', 'v3-date',
       'v3-time', 'v3-datetime', 'v2-min-type', 'v2-max-type', 'v2-minmax-type', 'v3-includes',
       'v3-null', 'v4-equals-ignore-order', 'v4-min-equals-ignore-order',
       'v4-max-equals-ignore-order', 'v4-minmax-equals-ignore-order', 'v3-content-type',
       'v4-array-contains', 'v1-equality', 'v4-not-empty', 'v4-semver'].collect {
        new CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, 'core', it)
      }
    )
  }

  private static String resolvedCoreKey(String entryKey) {
    def resolved = CatalogueManager.INSTANCE.resolveCapability(entryKey, CatalogueEntryType.MATCHER)
    assert resolved instanceof ResolvedCapability.Core
    ((ResolvedCapability.Core) resolved).key
  }

  def 'resolveCapability falls back to the versioned core key'() {
    given:
    registerCoreMatcherEntries()

    expect:
    // The name a Pact file (and a plugin calling back) uses, without needing to know which
    // specification version introduced the rule
    resolvedCoreKey(name) == expected

    where:
    name                  || expected
    'type'                || 'v2-type'
    'regex'               || 'v2-regex'
    'date'                || 'v3-date'
    'equality'            || 'v1-equality'
    'semver'              || 'v4-semver'
    'not-empty'           || 'v4-not-empty'
    // Only the whole name after the version prefix counts, so these are distinct rules and not
    // ambiguous with `type`/`equals-ignore-order`
    'content-type'        || 'v3-content-type'
    'min-type'            || 'v2-min-type'
    'equals-ignore-order' || 'v4-equals-ignore-order'
    // The versioned key itself still resolves, by name
    'v2-type'             || 'v2-type'
    'matcher/v3-date'     || 'v3-date'
    'core/matcher/v3-date' || 'v3-date'
  }

  def 'a key component is never matched as a substring'() {
    // "type" must not name core/matcher/v2-type - if it did, it would match all eight core keys
    // ending in "type" and be ambiguous. It resolves through the versioned fallback instead.
    expect:
    !CatalogueManagerKt.namesCatalogueKey('core/matcher/v2-type', 'type')
    CatalogueManagerKt.namesCatalogueKey('core/matcher/v2-type', 'v2-type')
    CatalogueManagerKt.namesCatalogueKey('core/matcher/v2-type', 'matcher/v2-type')
    CatalogueManagerKt.namesCatalogueKey('core/matcher/v2-type', 'core/matcher/v2-type')
    !CatalogueManagerKt.namesCatalogueKey('core/matcher/v2-type', 'r/v2-type')
    CatalogueManagerKt.namesCatalogueKey('core/content-matcher/xml', 'xml')
    !CatalogueManagerKt.namesCatalogueKey('core/content-matcher/xml', 'ml')
  }

  def 'the versioned fallback only applies to matcher and generator entries'() {
    given:
    // Content matchers, content generators and transports are registered under plain names, so a
    // leading "v<n>-" there is part of the name, not a version to be stripped.
    def key = 'versioned fallback entry types'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.CORE, 'core', "v2-$key"),
      new CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, 'core', "v2-matcher-$key")
    ])

    when:
    CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.CONTENT_MATCHER)

    then:
    // The content matcher is only reachable by its actual name
    thrown(PactCatalogueEntryNotFoundException)
    CatalogueManager.INSTANCE.lookupEntry(key) == null
    CatalogueManager.INSTANCE.resolveCapability("v2-$key", CatalogueEntryType.CONTENT_MATCHER) != null
    // ... while the matcher entry still gets the fallback
    resolvedCoreKey("matcher-$key") == "v2-matcher-$key"
  }

  def 'lookupEntry matches by name, not by substring'() {
    given:
    def key = 'lookupEntry-matches-by-name'
    def entry = Plugin.CatalogueEntry.newBuilder()
      .setTypeValue(CatalogueEntryType.CONTENT_MATCHER.toEntryValue())
      .setKey(key)
      .build()
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-lookup', [entry])

    expect:
    CatalogueManager.INSTANCE.lookupEntry(key)?.type == CatalogueEntryType.CONTENT_MATCHER
    CatalogueManager.INSTANCE.lookupEntry("content-matcher/$key")?.type == CatalogueEntryType.CONTENT_MATCHER
    CatalogueManager.INSTANCE
      .lookupEntry("plugin/CatalogueManagerSpec-lookup/content-matcher/$key")?.type == CatalogueEntryType.CONTENT_MATCHER
    // A trailing substring of a component names nothing
    CatalogueManager.INSTANCE.lookupEntry('matches-by-name') == null

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-lookup')
  }

  def 'lookupEntry falls back to the versioned core key'() {
    given:
    registerCoreMatcherEntries()

    expect:
    CatalogueManager.INSTANCE.lookupEntry('type')?.key == 'v2-type'
    CatalogueManager.INSTANCE.lookupEntry('v3-date')?.key == 'v3-date'
    CatalogueManager.INSTANCE.lookupEntry('matcher/v3-date')?.key == 'v3-date'
  }

  def 'resolveCapability prefers an entry named directly over the versioned fallback'() {
    given:
    def key = 'resolveCapability prefers an entry named directly'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, 'core', "v3-$key")
    ])
    // Before the plugin registers anything, the bare name finds the core rule via the fallback
    def coreFirst = resolvedCoreKey(key)

    def pluginEntry = Plugin.CatalogueEntry.newBuilder()
      .setTypeValue(CatalogueEntryType.MATCHER.toEntryValue())
      .setKey(key)
      .build()
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-versioned', [pluginEntry])

    when:
    def resolved = CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.MATCHER)

    then:
    coreFirst == "v3-$key"
    resolved instanceof ResolvedCapability.Plugin
    ((ResolvedCapability.Plugin) resolved).pluginName == 'CatalogueManagerSpec-versioned'
    // A caller that specifically wants the core rule can still name its versioned key
    resolvedCoreKey("v3-$key") == "v3-$key"

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-versioned')
  }

  def 'resolveCapability throws a clear error for an unregistered key'() {
    when:
    CatalogueManager.INSTANCE.resolveCapability(
      'resolveCapability throws a clear error for an unregistered key', CatalogueEntryType.CONTENT_MATCHER)

    then:
    thrown(PactCatalogueEntryNotFoundException)
  }

  def 'resolveCapability throws a clear error for the wrong capability shape'() {
    given:
    def key = 'resolveCapability throws a clear error for the wrong capability shape'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.CORE, '', key)
    ])

    when:
    CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.CONTENT_MATCHER)

    then:
    def ex = thrown(PactCatalogueEntryTypeMismatchException)
    ex.actualType == CatalogueEntryType.CONTENT_GENERATOR
    ex.expectedType == CatalogueEntryType.CONTENT_MATCHER
  }

  def 'resolveCapability rejects an ambiguous key shared by a core and a plugin entry'() {
    given:
    def key = 'resolveCapability rejects an ambiguous key shared by a core and a plugin entry'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.CORE, '', key)
    ])
    def pluginEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER)
      .setKey(key)
      .build()
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-ambiguous', [pluginEntry])

    when:
    CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.CONTENT_MATCHER)

    then:
    thrown(PactCatalogueEntryAmbiguousException)

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-ambiguous')
  }
}
