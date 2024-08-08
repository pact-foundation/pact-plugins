package io.pact.example.grpc.consumer;

import area_calculator.AreaCalculator;
import area_calculator.CalculatorGrpc;
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
import io.grpc.Metadata;
import io.grpc.stub.MetadataUtils;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;

import static area_calculator.CalculatorGrpc.newBlockingStub;
import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static io.grpc.Metadata.ASCII_STRING_MARSHALLER;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "area-calculator-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {

  @Pact(consumer = "grpc-consumer-jvm")
  V4Pact calculateRectangleArea(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .given("a rectangle")
      .expectsToReceive("calculate rectangle area request", "core/interaction/synchronous-message")
      .with(Map.of(
        "pact:proto", filePath("../proto/area_calculator.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Calculator/calculateOne",

        "request", Map.of(
          "rectangle", Map.of(
              "length", "matching(number, fromProviderState('${rectangleLength}', 3))",
              "width", "matching(number, fromProviderState('${rectangleWidth}', 3))"
          )),

          "requestMetadata", Map.of(
            "Auth", "matching(equalTo, fromProviderState('${Auth}', 'AST00004'))"
          ),

        "response", List.of(
            Map.of(
              "value", "matching(number, 12)"
            )
          )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(pactMethod = "calculateRectangleArea")
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void calculateRectangleArea(MockServer mockServer, V4Interaction.SynchronousMessages interaction) throws InvalidProtocolBufferException {
    io.grpc.Metadata.Key<String> authKey = io.grpc.Metadata.Key.of("Auth", ASCII_STRING_MARSHALLER);
    io.grpc.Metadata metadata = new io.grpc.Metadata();
    metadata.put(authKey, "AST00004");
    AtomicReference<Metadata> headers = new AtomicReference<>(null);
    AtomicReference<io.grpc.Metadata> trailers = new AtomicReference<>(null);
    ManagedChannel channel = ManagedChannelBuilder.forTarget("127.0.0.1:" + mockServer.getPort())
      .usePlaintext()
      .intercept(MetadataUtils.newAttachHeadersInterceptor(metadata))
      .intercept(MetadataUtils.newCaptureMetadataInterceptor(headers, trailers))
      .build();
    CalculatorGrpc.CalculatorBlockingStub stub = newBlockingStub(channel);

    // Correct request
    AreaCalculator.ShapeMessage shapeMessage = AreaCalculator.ShapeMessage.parseFrom(interaction.getRequest().getContents().getValue());
    AreaCalculator.AreaResponse response = stub.calculateOne(shapeMessage);
    assertThat(response.getValue(0), equalTo(12.0F));
  }
}
