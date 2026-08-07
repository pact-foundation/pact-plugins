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
   * The core matcher entries as the Pact frameworks actually register them - keyed by the name the
   * rule carries in a request (MatchingRule.name), with the specification version it was
   * introduced in as a value. Kept in sync with MatcherExecutor.kt in Pact-JVM and
   * MATCHER_CATALOGUE_ENTRIES in pact_matching.
   */
  private static void registerCoreMatcherEntries() {
    CatalogueManager.INSTANCE.registerCoreEntries(
      [equality: 'V1', regex: 'V2', type: 'V2', 'min-type': 'V2', 'max-type': 'V2',
       'min-max-type': 'V2', include: 'V3', number: 'V3', integer: 'V3', decimal: 'V3', null: 'V3',
       date: 'V3', time: 'V3', datetime: 'V3', 'content-type': 'V3', values: 'V3',
       'array-contains': 'V4', boolean: 'V4', 'status-code': 'V4', 'not-empty': 'V4', semver: 'V4',
       'each-key': 'V4', 'each-value': 'V4', 'ignore-order': 'V4', 'min-ignore-order': 'V4',
       'max-ignore-order': 'V4', 'min-max-ignore-order': 'V4'].collect { key, version ->
        new CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, 'core', key,
          ['spec-version': version])
      }
    )
  }

  private static String resolvedCoreKey(String entryKey) {
    def resolved = CatalogueManager.INSTANCE.resolveCapability(entryKey, CatalogueEntryType.MATCHER)
    assert resolved instanceof ResolvedCapability.Core
    ((ResolvedCapability.Core) resolved).key
  }

  def 'resolveCapability resolves a core rule by the name it is registered under'() {
    given:
    registerCoreMatcherEntries()

    expect:
    // The name a Pact file (and a plugin calling back) uses - the same string the driver puts in
    // MatchFieldRequest.rule.type, so a plugin can forward a rule it was handed straight back
    resolvedCoreKey(name) == expected

    where:
    name                   || expected
    'type'                 || 'type'
    'regex'                || 'regex'
    'date'                 || 'date'
    'equality'             || 'equality'
    'semver'               || 'semver'
    'not-empty'            || 'not-empty'
    // Rules whose name ends in another rule's name are distinct entries, not ambiguous with it
    'content-type'         || 'content-type'
    'min-type'             || 'min-type'
    'min-max-type'         || 'min-max-type'
    'ignore-order'         || 'ignore-order'
    // More of the catalogue key resolves the same entry
    'matcher/date'         || 'date'
    'core/matcher/date'    || 'date'
  }

  def 'a key component is never matched as a substring'() {
    // "type" names the `type` rule and nothing else - if a component matched by suffix it would
    // also name `content-type`, `min-type` and `max-type`, and be ambiguous across all of them
    expect:
    CatalogueManagerKt.namesCatalogueKey('core/matcher/type', 'type')
    !CatalogueManagerKt.namesCatalogueKey('core/matcher/content-type', 'type')
    CatalogueManagerKt.namesCatalogueKey('core/matcher/type', 'matcher/type')
    CatalogueManagerKt.namesCatalogueKey('core/matcher/type', 'core/matcher/type')
    !CatalogueManagerKt.namesCatalogueKey('core/matcher/type', 'r/type')
    CatalogueManagerKt.namesCatalogueKey('core/content-matcher/xml', 'xml')
    !CatalogueManagerKt.namesCatalogueKey('core/content-matcher/xml', 'ml')
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

  def 'lookupEntry finds a core rule by name'() {
    given:
    registerCoreMatcherEntries()

    expect:
    CatalogueManager.INSTANCE.lookupEntry('type')?.key == 'type'
    CatalogueManager.INSTANCE.lookupEntry('date')?.key == 'date'
    CatalogueManager.INSTANCE.lookupEntry('matcher/date')?.key == 'date'
    CatalogueManager.INSTANCE.lookupEntry('core/matcher/date')?.key == 'date'
  }

  def 'resolveCapability reports a plugin rule sharing a core rule name as ambiguous'() {
    given:
    // Core rules are now keyed by the rule name itself, so a plugin registering the same name is a
    // genuine collision. Neither wins silently - both are reachable by a fully qualified key.
    def key = 'resolveCapability plugin rule sharing a core rule name'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, 'core', key)
    ])
    // Before the plugin registers anything, the bare name finds the core rule
    def coreFirst = resolvedCoreKey(key)

    def pluginEntry = Plugin.CatalogueEntry.newBuilder()
      .setTypeValue(CatalogueEntryType.MATCHER.toEntryValue())
      .setKey(key)
      .build()
    CatalogueManager.INSTANCE.registerPluginEntries('CatalogueManagerSpec-shared-name', [pluginEntry])

    when:
    CatalogueManager.INSTANCE.resolveCapability(key, CatalogueEntryType.MATCHER)

    then:
    coreFirst == key
    def ex = thrown(PactCatalogueEntryAmbiguousException)
    ex.matchingKeys == ["core/matcher/$key".toString(),
                        "plugin/CatalogueManagerSpec-shared-name/matcher/$key".toString()]
    // Each is still reachable by a fully qualified key
    resolvedCoreKey("core/matcher/$key") == key
    CatalogueManager.INSTANCE.resolveCapability(
      "plugin/CatalogueManagerSpec-shared-name/matcher/$key",
      CatalogueEntryType.MATCHER) instanceof ResolvedCapability.Plugin

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec-shared-name')
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
