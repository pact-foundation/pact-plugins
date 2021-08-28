package io.pact.plugins.jvm.core

import au.com.dius.pact.consumer.MockServer
import au.com.dius.pact.consumer.dsl.PactBuilder
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt
import au.com.dius.pact.consumer.junit5.PactTestFor
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.model.annotations.Pact
import io.grpc.ManagedChannelBuilder
import org.junit.jupiter.api.Disabled
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

import static io.pact.plugin.PactPluginGrpc.newBlockingStub

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = 'plugin', pactVersion = PactSpecVersion.V4)
@Disabled("Not implemented yet")
class DriverPactTest {

  @Pact(consumer = 'pact-jvm-driver')
  V4Pact initInteraction(PactBuilder builder) {
    return builder
      .usingPlugin('protobuf')
      .expectsToReceive('init plugin request', 'plugin/protobuf/interaction/unary')
      .with([
        'proto': '../../../proto/',
        'service': 'io.pact.plugin.PactPlugin/InitPlugin',
        'request': [
          'message': 'InitPluginRequest',
          'implementation': "notEmpty('pact-jvm-driver')",
          'version': "matching(semver, '0.0.0')"
        ],
        'response': [
          'message': 'InitPluginResponse',
          'catalogue': 'eachLike(CatalogueEntry)',
          'CatalogueEntry': [
            'type': "notEmpty('content-matcher')",
            'key': "notEmpty('csv')"
          ]
        ]
      ])
      .toPact()
  }

  @Test
  @PactTestFor(pactMethod = "initInteraction")
  void initInteractionTest(MockServer mockServer) {
    def channel = ManagedChannelBuilder.forTarget(mockServer.url)
      .usePlaintext()
      .build()
    def stub = newBlockingStub(channel)
    def response = DefaultPluginManager.INSTANCE.makeInitRequest(stub)
    assert !response.catalogueList.isEmpty()
  }
}
