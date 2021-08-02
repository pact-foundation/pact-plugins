package io.grpc.examples.csv;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.Map;

@ExtendWith(PactConsumerTestExt.class)
class CsvClientTest {
  @Pact(consumer = "CsvClient")
  V4Pact pact(PactBuilder builder) {
    return builder
      .usingPlugin("csv")
      .expectsToReceive("request for a report", "core/interaction/http")
      .with(Map.of("path", "/reports/report001.csv"))
      .willRespondWith(Map.of(
        "status", "200",
        "body", Map.of(
          "Content-Type", "application/csv",
          "column:1", "matching(type,'Name')",
          "column:2", "matching(number,100)",
          "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "CsvServer")
  void test(MockServer mockServer) {
    CsvClient client = new CsvClient(mockServer.getUrl());
    client.fetch("report001.csv");
  }
}
