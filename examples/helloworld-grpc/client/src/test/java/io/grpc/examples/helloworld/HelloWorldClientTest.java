package io.grpc.examples.helloworld;

import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import io.grpc.Channel;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.Map;

@ExtendWith(PactConsumerTestExt.class)
class HelloWorldClientTest {
  @Pact(consumer = "HelloWorldClient")
  V4Pact pact1(PactBuilder builder) {
    return builder
      .usingPlugin("grpc")
      .withPluginConfig("grpc", Map.of("proto", "examples/helloworld/proto"))
      .withPluginDSL(GrpcBuilder
        .expectsToReceive("a hello world message")
        .request("HelloRequest", b -> b.attribute("name", matching("\\w+", "World")))
        .response("HelloReply", b -> b.attribute("message", matching("Hello \\w+", "Hello World")))
      )
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "HelloWorldServer", pactMethod = "pact1")
  void test1(Channel channel) {
    HelloWorldClient client = new HelloWorldClient(channel);
    client.greet("bob");
  }

  @Pact(consumer = "HelloWorldClient")
  V4Pact pact2(PactBuilder builder) {
    return builder
      .usingPlugin("grpc")
      .withPluginConfig("grpc", Map.of("proto", "examples/helloworld/proto"))
      .expectsToReceive("a hello world message", "grpc/interaction/req->res")
      .with(Map.of("HelloRequest.name", "matching('\\w+', 'World')"))
      .willRespondWith(Map.of("HelloReply.message", "matching('Hello \\w+', 'Hello World')"))
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "HelloWorldServer", pactMethod = "pact2")
  void test2(Channel channel) {
    HelloWorldClient client = new HelloWorldClient(channel);
    client.greet("bob");
  }
}
