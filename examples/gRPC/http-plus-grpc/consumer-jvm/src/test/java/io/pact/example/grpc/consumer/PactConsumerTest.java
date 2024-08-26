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
import org.apache.hc.client5.http.fluent.Request;
import org.apache.hc.core5.http.ClassicHttpResponse;
import org.apache.hc.core5.http.ContentType;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.io.IOException;
import java.util.List;
import java.util.Map;

import static area_calculator.CalculatorGrpc.newBlockingStub;
import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;
import static org.hamcrest.Matchers.is;

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "area-calculator-provider", pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {

  @Pact(consumer = "area-calculator-consumer")
  V4Pact calculateRectangleArea(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("calculate rectangle area request", "core/interaction/synchronous-message")
      .with(Map.of(
        "pact:proto", filePath("../proto/area_calculator.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Calculator/calculateOne",

        "request", Map.of(
          "rectangle", Map.of(
              "length", "matching(number, 3)",
              "width", "matching(number, 4)"
          )),

        "response", List.of(
            Map.of(
              "value", "matching(number, 12)"
            )
          )
      ))
      .toPact();
  }

  @Pact(consumer = "area-calculator-consumer")
  V4Pact calculateRectangleAreaHttp(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("calculate rectangle area request via HTTP", "")
      .with(Map.of(
        "request.path", "/Calculator/calculateOne",
        "request.method", "POST",
        "request.contents", Map.of(
          "pact:proto", filePath("../proto/area_calculator.proto"),
          "pact:content-type", "application/protobuf;message=.area_calculator.ShapeMessage",
          "pact:message-type", "ShapeMessage",
          "rectangle", Map.of(
            "length", "matching(number, 3)",
            "width", "matching(number, 4)"
          )
        ),
        "response.status", "200",
        "response.contents", Map.of(
          "pact:proto", filePath("../proto/area_calculator.proto"),
          "pact:content-type", "application/protobuf;message=.area_calculator.AreaResponse",
          "pact:message-type", "AreaResponse",
          "value", "matching(number, 12)"
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(pactMethod = "calculateRectangleArea", providerType = ProviderType.SYNCH_MESSAGE)
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void calculateRectangleArea(MockServer mockServer, V4Interaction.SynchronousMessages interaction) throws InvalidProtocolBufferException {
    ManagedChannel channel = ManagedChannelBuilder.forTarget("127.0.0.1:" + mockServer.getPort())
      .usePlaintext()
      .build();
    CalculatorGrpc.CalculatorBlockingStub stub = newBlockingStub(channel);

    AreaCalculator.ShapeMessage shapeMessage = AreaCalculator.ShapeMessage.parseFrom(interaction.getRequest().getContents().getValue());
    AreaCalculator.AreaResponse response = stub.calculateOne(shapeMessage);
    assertThat(response.getValue(0), equalTo(12.0F));
  }

  @Test
  @PactTestFor(pactMethod = "calculateRectangleAreaHttp", providerType = ProviderType.SYNCH)
  void calculateRectangleAreaHttp(MockServer mockServer) throws IOException {
    AreaCalculator.ShapeMessage shapeMessage = AreaCalculator.ShapeMessage
      .newBuilder()
      .setRectangle(AreaCalculator.Rectangle.newBuilder()
        .setWidth(5f)
        .setLength(10f)
        .build()
      )
      .build();
    ClassicHttpResponse httpResponse = (ClassicHttpResponse) Request.post(mockServer.getUrl() + "/Calculator/calculateOne")
      .bodyByteArray(shapeMessage.toByteArray(), ContentType.parse("application/protobuf;message=.area_calculator.ShapeMessage"))
      .execute()
      .returnResponse();
    assertThat(httpResponse.getCode(), is(equalTo(200)));
    AreaCalculator.AreaResponse response = AreaCalculator.AreaResponse.parseFrom(httpResponse.getEntity().getContent());
    assertThat(response.getValue(0), equalTo(12f));
  }
}
