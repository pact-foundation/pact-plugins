package io.pact.example.grpc.consumer;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit.MockServerConfig;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.consumer.junit5.ProviderType;
import au.com.dius.pact.consumer.model.MockServerImplementation;
import au.com.dius.pact.core.model.PactSpecVersion;
import au.com.dius.pact.core.model.V4Interaction;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import com.google.protobuf.InvalidProtocolBufferException;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import routeguide.other.Enum;
import routeguide.v2.TestEnum;
import routeguide.v2.TestGrpc;

import java.util.List;
import java.util.Map;

import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;
import static routeguide.v2.TestGrpc.newBlockingStub;

/**
 * Main test class for the test_enum service method call.
 */
@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "test-enum-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {

  @Pact(consumer = "grpc-consumer-jvm")
  V4Pact testEnumPact(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("test enum request", "core/interaction/synchronous-message")
      .with(Map.of(
        // Configure the proto file, the content type and the service we expect to invoke
        "pact:proto", filePath("../proto/test_enum.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Test/GetFeature2",
        "pact:protobuf-config", Map.of(
          "additionalIncludes", List.of(filePath("../proto2"))
        ),

        "request", Map.of(
          "latitude", "matching(number, 3)",
          "longitude", "matching(number, 4)"
        ),
        "response", Map.of(
          "result", "matching(type, 'VALUE_ONE')"
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(pactMethod = "testEnumPact")
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void testEnum(MockServer mockServer, V4Interaction.SynchronousMessages interaction) throws InvalidProtocolBufferException {
    ManagedChannel channel = ManagedChannelBuilder.forTarget("127.0.0.1:" + mockServer.getPort())
      .usePlaintext()
      .build();
    TestGrpc.TestBlockingStub stub = newBlockingStub(channel);

    // Correct request
    TestEnum.Point point = TestEnum.Point.parseFrom(interaction.getRequest().getContents().getValue());
    TestEnum.Feature response = stub.getFeature2(point);
    assertThat(response.getResult(), equalTo(Enum.OtherFileEnum.VALUE_ONE));
  }
}
