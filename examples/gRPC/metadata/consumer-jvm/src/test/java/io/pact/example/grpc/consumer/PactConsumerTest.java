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
import io.grpc.stub.MetadataUtils;
import metadatatest.Metadata;
import metadatatest.TestGrpc;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;

import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static io.grpc.Metadata.ASCII_STRING_MARSHALLER;
import static metadatatest.TestGrpc.newBlockingStub;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "validate-token-provider", providerType = ProviderType.SYNCH_MESSAGE, pactVersion = PactSpecVersion.V4)
public class PactConsumerTest {
  @Pact(consumer = "grpc-consumer-jvm")
  V4Pact testMetadataPact(PactBuilder builder) {
    return builder
      .usingPlugin("protobuf")
      .expectsToReceive("validate token request", "core/interaction/synchronous-message")
      .with(Map.of(
        "pact:proto", filePath("../proto/metadata.proto"),
        "pact:content-type", "application/grpc",
        "pact:proto-service", "Test/ValidateToken",

        "request", Map.of(),
        "requestMetadata", Map.of(
          "Auth", "matching(regex, '[A-Z]{3}\\d+', 'AST00004')"
        ),

        "response", Map.of(
          "ok", "matching(boolean, true)"
        ),
        "responseMetadata", Map.of(
          "code", "matching(integer, 100)"
        )
      ))
      .toPact();
  }

  @Test
  @MockServerConfig(implementation = MockServerImplementation.Plugin, registryEntry = "protobuf/transport/grpc")
  void testMetadata(MockServer mockServer) {
    io.grpc.Metadata.Key<String> authKey = io.grpc.Metadata.Key.of("Auth", ASCII_STRING_MARSHALLER);
    io.grpc.Metadata.Key<String> codeKey = io.grpc.Metadata.Key.of("code", ASCII_STRING_MARSHALLER);

    io.grpc.Metadata metadata = new io.grpc.Metadata();
    metadata.put(authKey, "ABC123");
    AtomicReference<io.grpc.Metadata> headers = new AtomicReference<>(null);
    AtomicReference<io.grpc.Metadata> trailers = new AtomicReference<>(null);
    ManagedChannel channel = ManagedChannelBuilder.forTarget("127.0.0.1:" + mockServer.getPort())
      .usePlaintext()
      .intercept(MetadataUtils.newAttachHeadersInterceptor(metadata))
      .intercept(MetadataUtils.newCaptureMetadataInterceptor(headers, trailers))
      .build();
    TestGrpc.TestBlockingStub stub = newBlockingStub(channel);

    Metadata.ValidateTokenRequest request = Metadata.ValidateTokenRequest.newBuilder().build();
    Metadata.ValidateTokenResult response = stub.validateToken(request);
    assertThat(response.getOk(), equalTo(true));
    assertThat(headers.get().get(codeKey), equalTo("100"));
  }
}
