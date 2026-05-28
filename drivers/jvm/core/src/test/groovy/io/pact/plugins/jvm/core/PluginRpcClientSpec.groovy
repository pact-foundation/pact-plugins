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
    def request = Plugin.InitPluginRequest.newBuilder()
      .setImplementation('plugin-driver-jvm')
      .setVersion('1.0.0-beta.1')
      .build()
    def response = PluginV2.InitPluginResponse.newBuilder()
      .addCatalogue(PluginV2.CatalogueEntry.newBuilder()
        .setType(PluginV2.CatalogueEntry.EntryType.CONTENT_MATCHER)
        .setKey('test'))
      .build()
    ArgumentCaptor<PluginV2.InitPluginRequest> argument = ArgumentCaptor.forClass(PluginV2.InitPluginRequest)
    doReturn(response).when(stub).initPlugin(argument.capture())

    when:
    def result = client.initPlugin(request)

    then:
    argument.value.implementation == 'plugin-driver-jvm'
    argument.value.version == '1.0.0-beta.1'
    result.catalogueCount == 1
    result.catalogueList[0].key == 'test'
    result.catalogueList[0].type == Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER
  }
}
