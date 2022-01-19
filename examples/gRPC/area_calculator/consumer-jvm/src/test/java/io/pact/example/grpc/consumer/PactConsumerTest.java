package io.pact.example.grpc.consumer;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit.MockServerConfig;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.consumer.junit5.ProviderType;
import au.com.dius.pact.consumer.model.MockServerImplementation;
import au.com.dius.pact.core.model.PactSpecVersion;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.List;
import java.util.Map;

import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "area-calculator-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {
  @Pact(consumer = "protobuf-consumer")
  V4Pact calculateRectangleArea(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("calculate rectangle area request", "core/interaction/synchronous-message")
      .with(Map.of(
        "pact:proto", filePath("../../../proto/plugin.proto"),
        "pact:content-type", "application/protobuf",
        "pact:proto-service", "Calculator/calculate",
        "request", Map.of(
          "rectangle", Map.of(
              "length", "matching(number, 3)",
              "width", "matching(number, 4)"
          ),
          "response", List.of(
            Map.of(
              "value", "matching(number, 12)"
            )
          )
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(pactMethod = "calculateRectangleArea")
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/mock-server/grpc")
  void consumeInitPluginMessage(MockServer mockServer) {
//    Plugin.InitPluginRequest request = Plugin.InitPluginRequest.parseFrom(message.getContents().getContents().getValue());
//    assertThat(request.getImplementation(), is("pact-jvm-driver"));
//    assertThat(request.getVersion(), is("0.0.0"));
  }
}
