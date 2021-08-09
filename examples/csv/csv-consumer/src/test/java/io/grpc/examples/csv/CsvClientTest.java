package io.grpc.examples.csv;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.io.IOException;
import java.util.Map;

import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;
import static org.hamcrest.Matchers.is;
import static org.hamcrest.Matchers.matchesRegex;

@ExtendWith(PactConsumerTestExt.class)
class CsvClientTest {
  @Pact(consumer = "CsvClient")
  V4Pact pact(PactBuilder builder) {
    return builder
      .usingPlugin("csv")
      .expectsToReceive("request for a report", "core/interaction/http")
      .with(Map.of("request.path", "/reports/report001.csv"))
      .willRespondWith(Map.of(
        "status", "200",
        "contents", Map.of(
          "content-type", "text/csv",
          "column:1", "matching(type,'Name')",
          "column:2", "matching(number,100)",
          "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "CsvServer")
  void test(MockServer mockServer) throws IOException {
    CsvClient client = new CsvClient(mockServer.getUrl());
    String csv = client.fetch("report001.csv");
    String[] values = csv.trim().split(",");
    assertThat(values[0], is(equalTo("Name")));
    assertThat(values[1], is(equalTo("100")));
    assertThat(values[2], matchesRegex("\\d{4}-\\d{2}-\\d{2}"));
  }
}
