package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import spock.lang.Specification

class CatalogueManagerSpec extends Specification {
  def 'sets plugin catalogue entries correctly'() {
    given:
    def matcherEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER)
      .setKey('protobuf-test')
      .putValues('content-types', 'application/protobuf;application/grpc')
      .build()
    def generatorEntry = Plugin.CatalogueEntry.newBuilder()
      .setType(Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR)
      .setKey('protobuf-test')
      .putValues('content-types', 'application/protobuf;application/grpc')
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
      'CatalogueManagerSpec', 'protobuf-test', ['content-types': 'application/protobuf;application/grpc'])
    contentGenerator == new CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec', 'protobuf-test', ['content-types': 'application/protobuf;application/grpc'])
    transport == new CatalogueEntry(CatalogueEntryType.TRANSPORT, CatalogueEntryProviderType.PLUGIN,
      'CatalogueManagerSpec', 'grpc-test')

    cleanup:
    CatalogueManager.INSTANCE.removePluginEntries('CatalogueManagerSpec')
  }
}
