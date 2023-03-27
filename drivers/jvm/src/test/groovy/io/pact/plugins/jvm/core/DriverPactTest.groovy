package io.pact.plugins.jvm.core

import au.com.dius.pact.consumer.dsl.PactBuilder
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt
import au.com.dius.pact.consumer.junit5.PactTestFor
import au.com.dius.pact.consumer.junit5.ProviderType
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.V4Interaction
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.model.annotations.Pact
import io.grpc.ManagedChannel
import io.grpc.stub.AbstractBlockingStub
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import org.jetbrains.annotations.Nullable
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith
import org.mockito.ArgumentCaptor
import org.mockito.Mockito

import java.util.function.Function

import static org.mockito.Mockito.doReturn

/**
 * This is a Pact test for the JVM driver to any plugin implementing version 1 of the plugin interface
 */
@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = 'plugin', pactVersion = PactSpecVersion.V4, providerType = ProviderType.SYNCH_MESSAGE)
class DriverPactTest {

  /*
   * Mock plugin to mock out the gRPC details for the test
   */
  static class MockPlugin implements PactPlugin {
    Plugin.InitPluginRequest request
    Plugin.InitPluginResponse response
    PactPluginManifest manifest = [getName: () -> 'test-plugin'] as PactPluginManifest

    @Override
    PactPluginManifest getManifest() {
      manifest
    }

    @Override
    Integer getPort() {
      null
    }

    @Override
    String getServerKey() {
      null
    }

    @Override
    Long getProcessPid() {
      null
    }

    @Override
    AbstractBlockingStub<PactPluginGrpc.PactPluginBlockingStub> getStub() {
      Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
    }

    @Override
    void setStub(@Nullable AbstractBlockingStub<PactPluginGrpc.PactPluginBlockingStub> stub) {

    }

    @Override
    List<Plugin.CatalogueEntry> getCatalogueEntries() {
      null
    }

    @Override
    void setCatalogueEntries(@Nullable List<Plugin.CatalogueEntry> catalogueEntries) {

    }

    @Override
    ManagedChannel getChannel() {
      null
    }

    @Override
    void setChannel(@Nullable ManagedChannel channel) {

    }

    @Override
    void shutdown() {

    }

    /*
     * This is the method that the Plugin Manager will use to make the gRPC call. We can mock out that gRPC stub here
     */
    @Override
    <T> T withGrpcStub(Function<PactPluginGrpc.PactPluginBlockingStub, T> callback) {
      def mock = Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
      ArgumentCaptor<Plugin.InitPluginRequest> argument = ArgumentCaptor.forClass(Plugin.InitPluginRequest.class)
      doReturn(response).when(mock).initPlugin(argument.capture())
      def result = callback(mock)

      assert argument.value.implementation == request.implementation

      result
    }
  }

  /*
   * Init plugin request interaction which is sent when the plugin is first loaded
   */
  @Pact(consumer = 'pact-jvm-driver')
  V4Pact initInteraction(PactBuilder builder) {
    return builder
      .usingPlugin('protobuf')
      .expectsToReceive('init plugin request', 'core/interaction/synchronous-message')
      .with([
        'pact:proto': PactBuilder.filePath("../../proto/plugin.proto"),
        'pact:content-type': 'application/protobuf',
        'pact:proto-service': 'PactPlugin/InitPlugin',
        'request': [
          'implementation': "notEmpty('plugin-driver-jvm')",
          'version': "matching(semver, '0.0.0')"
        ],
        'response': [
          'catalogue': [
            'pact:match' : "eachValue(matching(\$'CatalogueEntry'))",
            'CatalogueEntry': [
              'type': "matching(regex, 'CONTENT_MATCHER|CONTENT_GENERATOR|TRANSPORT', 'CONTENT_MATCHER')",
              'key': "notEmpty('test')"
            ]
          ]
        ]
      ])
      .toPact()
  }

  /*
   * Test for the init plugin call
   */
  @Test
  @PactTestFor(pactMethod = "initInteraction")
  void initInteractionTest(V4Interaction.SynchronousMessages message) {
    // Get the request and response from the Pact, and use that to setup the mock gRPC call
    Plugin.InitPluginRequest requestMessage = Plugin.InitPluginRequest.parseFrom(message.request.contents.value)
    Plugin.InitPluginResponse responseMessage = Plugin.InitPluginResponse.parseFrom(message.response.first().contents.value)
    def plugin = new MockPlugin(request: requestMessage, response: responseMessage)

    // Init plugin call
    DefaultPluginManager.INSTANCE.initPlugin(plugin)

    // Check that the catalogue was updated with the entry from the test
    assert CatalogueManager.INSTANCE.lookupEntry('plugin/test-plugin/content-matcher/test') != null
  }
}
