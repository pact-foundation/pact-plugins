package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import io.pact.plugin.v2.PactPluginGrpc as PactPluginGrpcV2
import io.pact.plugin.v2.PluginV2
import org.mockito.ArgumentCaptor
import org.mockito.Mockito
import spock.lang.Specification

import static org.mockito.Mockito.doReturn

class PluginRpcClientSpec extends Specification {

  def 'v2 rpc client translates init plugin messages between v1 and v2 types'() {
    given:
    def stub = Mockito.mock(PactPluginGrpcV2.PactPluginBlockingStub)
    def client = new PactPluginV2RpcClient(stub)
    def request = new PluginInitRequest('plugin-driver-jvm', '1.0.0-beta.1', ['content-matcher/test'])
    def response = PluginV2.InitPluginResponse.newBuilder()
      .setSuccess(PluginV2.InitPluginSuccess.newBuilder()
        .addCatalogue(PluginV2.CatalogueEntry.newBuilder()
          .setType(PluginV2.CatalogueEntry.EntryType.CONTENT_MATCHER)
          .setKey('test'))
        .addPluginCapabilities('content-matcher/test'))
      .build()
    ArgumentCaptor<PluginV2.InitPluginRequest> argument = ArgumentCaptor.forClass(PluginV2.InitPluginRequest)
    doReturn(response).when(stub).initPlugin(argument.capture())

    when:
    def result = client.initPlugin(request)

    then:
    argument.value.implementation == 'plugin-driver-jvm'
    argument.value.version == '1.0.0-beta.1'
    argument.value.hostCapabilitiesList == ['content-matcher/test']
    result.catalogueEntries.size() == 1
    result.catalogueEntries[0].key == 'test'
    result.catalogueEntries[0].type == Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER
    result.pluginCapabilities == ['content-matcher/test']
  }

  def 'v2 rpc client raises an error for missing host capabilities'() {
    given:
    def stub = Mockito.mock(PactPluginGrpcV2.PactPluginBlockingStub)
    def client = new PactPluginV2RpcClient(stub)
    def request = new PluginInitRequest('plugin-driver-jvm', '1.0.0-beta.1', [])
    def response = PluginV2.InitPluginResponse.newBuilder()
      .setFailure(PluginV2.InitPluginFailure.newBuilder()
        .setError('Missing required host capabilities')
        .addMissingHostCapabilities('content-matcher/test'))
      .build()
    doReturn(response).when(stub).initPlugin(Mockito.any())

    when:
    client.initPlugin(request)

    then:
    def ex = thrown(IllegalStateException)
    ex.message == 'Missing required host capabilities (missing host capabilities: content-matcher/test)'
  }
}
