package io.pact.plugins.jvm.core

import io.grpc.Status
import io.grpc.StatusRuntimeException
import io.grpc.stub.StreamObserver
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import spock.lang.Specification

class RecordingStreamObserver<T> implements StreamObserver<T> {
  T value
  Throwable error
  boolean completed

  @Override
  void onNext(T value) {
    this.value = value
  }

  @Override
  void onError(Throwable t) {
    this.error = t
  }

  @Override
  void onCompleted() {
    completed = true
  }
}

class PluginHostServerSpec extends Specification {
  def service = new PluginHostGrpcService()

  private static Status.Code statusCodeOf(Throwable error) {
    ((StatusRuntimeException) error).status.code
  }

  def 'compareContents dispatches to a registered core handler'() {
    given:
    def key = 'compareContents dispatches to a registered core handler'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.CORE, '', key)
    ])
    CoreCapabilityRegistry.INSTANCE.registerContentMatcher(key,
      { Plugin.CompareContentsRequest req -> Plugin.CompareContentsResponse.newBuilder().build() } as CoreContentMatcher)
    def request = PluginV2.HostCompareContentsRequest.newBuilder()
      .setEntryKey(key)
      .setRequest(PluginV2.CompareContentsRequest.newBuilder().build())
      .build()
    def observer = new RecordingStreamObserver<PluginV2.CompareContentsResponse>()

    when:
    service.compareContents(request, observer)

    then:
    observer.error == null
    observer.completed
    observer.value != null

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterContentMatcher(key)
  }

  def 'compareContents returns NOT_FOUND for an unknown entry key'() {
    given:
    def request = PluginV2.HostCompareContentsRequest.newBuilder()
      .setEntryKey('compareContents returns NOT_FOUND for an unknown entry key')
      .setRequest(PluginV2.CompareContentsRequest.newBuilder().build())
      .build()
    def observer = new RecordingStreamObserver<PluginV2.CompareContentsResponse>()

    when:
    service.compareContents(request, observer)

    then:
    observer.error != null
    statusCodeOf(observer.error) == Status.Code.NOT_FOUND
  }

  def 'compareContents returns NOT_FOUND for an entry of the wrong capability shape'() {
    given:
    def key = 'compareContents returns NOT_FOUND for an entry of the wrong capability shape'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.CORE, '', key)
    ])
    def request = PluginV2.HostCompareContentsRequest.newBuilder()
      .setEntryKey(key)
      .setRequest(PluginV2.CompareContentsRequest.newBuilder().build())
      .build()
    def observer = new RecordingStreamObserver<PluginV2.CompareContentsResponse>()

    when:
    service.compareContents(request, observer)

    then:
    observer.error != null
    statusCodeOf(observer.error) == Status.Code.NOT_FOUND
  }

  def 'generateContent dispatches to a registered core handler'() {
    given:
    def key = 'generateContent dispatches to a registered core handler'
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.CORE, '', key)
    ])
    CoreCapabilityRegistry.INSTANCE.registerContentGenerator(key,
      { Plugin.GenerateContentRequest req -> Plugin.GenerateContentResponse.newBuilder().build() } as CoreContentGenerator)
    def request = PluginV2.HostGenerateContentRequest.newBuilder()
      .setEntryKey(key)
      .setRequest(PluginV2.GenerateContentRequest.newBuilder().build())
      .build()
    def observer = new RecordingStreamObserver<PluginV2.GenerateContentResponse>()

    when:
    service.generateContent(request, observer)

    then:
    observer.error == null
    observer.completed
    observer.value != null

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterContentGenerator(key)
  }

  def 'generateContent returns NOT_FOUND for an unknown entry key'() {
    given:
    def request = PluginV2.HostGenerateContentRequest.newBuilder()
      .setEntryKey('generateContent returns NOT_FOUND for an unknown entry key')
      .setRequest(PluginV2.GenerateContentRequest.newBuilder().build())
      .build()
    def observer = new RecordingStreamObserver<PluginV2.GenerateContentResponse>()

    when:
    service.generateContent(request, observer)

    then:
    observer.error != null
    statusCodeOf(observer.error) == Status.Code.NOT_FOUND
  }
}
