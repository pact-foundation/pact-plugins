package io.pact.example.grpc.consumer;

import area_calculator.CalculatorGrpc;
import area_calculator.GrpcStatus;
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
import io.grpc.Status;
import io.grpc.StatusRuntimeException;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.Map;

import static area_calculator.CalculatorGrpc.newBlockingStub;
import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;

/**
 * This example simulates the Parallelogram shape not being implemented, and an UNIMPLEMENTED status is returned
 */
@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "grpc-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {
  @Pact(consumer = "grpc-consumer-jvm")
  V4Pact testMetadataPact(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("invalid request", "core/interaction/synchronous-message")
      .with(Map.of(
        "pact:proto", filePath("../proto/grpc_status.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Calculator/calculate",

        "request", Map.of(
          "parallelogram", Map.of(
            "base_length", "matching(number, 3)",
            "height", "matching(number, 4)"
          )
        ),

        "responseMetadata", Map.of(
          "grpc-status", "UNIMPLEMENTED",
          "grpc-message", "matching(type, 'we do not currently support parallelograms')"
        )
      ))
      .toPact();
  }

  @Test()
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void testMetadata(MockServer mockServer, V4Interaction.SynchronousMessages interaction) throws InvalidProtocolBufferException {
    ManagedChannel channel = ManagedChannelBuilder.forTarget("127.0.0.1:" + mockServer.getPort())
      .usePlaintext()
      .build();
    CalculatorGrpc.CalculatorBlockingStub stub = newBlockingStub(channel);

    StatusRuntimeException exception = Assertions.assertThrows(StatusRuntimeException.class, () -> {
      GrpcStatus.ShapeMessage shapeMessage = GrpcStatus.ShapeMessage.parseFrom(interaction.getRequest().getContents().getValue());
      stub.calculate(shapeMessage);
    });
    Assertions.assertEquals(Status.Code.UNIMPLEMENTED, exception.getStatus().getCode());
    Assertions.assertEquals("we do not currently support parallelograms", exception.getStatus().getDescription());
  }
}
