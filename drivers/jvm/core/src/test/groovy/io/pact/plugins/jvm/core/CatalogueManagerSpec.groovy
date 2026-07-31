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
