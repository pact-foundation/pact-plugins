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
