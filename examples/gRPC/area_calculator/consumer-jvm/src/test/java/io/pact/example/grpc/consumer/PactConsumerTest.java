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
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.List;
import java.util.Map;

import static area_calculator.CalculatorGrpc.newBlockingStub;
import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;

/**
 * Main test class for the AreaCalculator calculate service method call.
 */
@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "area-calculator-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {

  /**
   * Configures the Pact interaction for the test. This will load the Protobuf plugin, which will provide all the
   * Protobuf and gRPC support to the Pact framework.
   */
  @Pact(consumer = "protobuf-consumer")
  V4Pact calculateRectangleArea(PactBuilder builder) {
    return builder
      // Tell Pact we need the Protobuf plugin
      .usingPlugin("protobuf", "0.1.0")
      // We will use a V4 synchronous message interaction for the test
      .expectsToReceive("calculate rectangle area request", "core/interaction/synchronous-message")
      // We need to pass all the details for the interaction over to the plugin
      .with(Map.of(
        // Configure the proto file, the content type and the service we expect to invoke
        "pact:proto", filePath("../proto/area_calculator.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Calculator/calculateOne",

        // Details on the request message (ShapeMessage) we will send
        "request", Map.of(
          "rectangle", Map.of(
              "length", "matching(number, 3)",
              "width", "matching(number, 4)"
          )),

        // Details on the response message we expect to get back (AreaResponse)
        "response", List.of(
            Map.of(
              "value", "matching(number, 12)"
            )
          )
      ))
      .toPact();
  }

  /**
   * Main test method. This method will receive a gRPC mock server and example request message, which we will use the
   * generated stub classes to send to the mock server. The mock server will return the AreaResponse message configured
   * from the values in the setup method above.
   */
  @Test
  @PactTestFor(pactMethod = "calculateRectangleArea")
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void calculateRectangleArea(MockServer mockServer, V4Interaction.SynchronousMessages interaction) throws InvalidProtocolBufferException {
    ManagedChannel channel = ManagedChannelBuilder.forTarget("[::1]:" + mockServer.getPort())
      .usePlaintext()
      .build();
    CalculatorGrpc.CalculatorBlockingStub stub = newBlockingStub(channel);

    // Correct request
    AreaCalculator.ShapeMessage shapeMessage = AreaCalculator.ShapeMessage.parseFrom(interaction.getRequest().getContents().getValue());
    AreaCalculator.AreaResponse response = stub.calculateOne(shapeMessage);
    assertThat(response.getValue(0), equalTo(12.0F));

    // Incorrect request, missing the length field. Uncommenting this will cause the test to fail.
    //AreaCalculator.ShapeMessage.Builder builder = AreaCalculator.ShapeMessage.newBuilder();
    //AreaCalculator.Rectangle rectangle = builder.getRectangleBuilder().setWidth(22).build();
    //shapeMessage = builder.setRectangle(rectangle).build();
    //stub.calculateOne(shapeMessage);
  }
}
