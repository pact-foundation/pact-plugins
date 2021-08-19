package io.grpc.examples.csv;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import com.github.javafaker.Faker;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.io.IOException;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;

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
      .with(Map.of(
        "request.path", "/reports/report001.csv",
        "response.status", "200",
        "response.contents", Map.of(
          "content-type", "text/csv",
          "column:1", "matching(type,'Name')",
          "column:2", "matching(number,100)",
          "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
        )
      ))
      .toPact();
  }

  @Pact(consumer = "CsvClient")
  V4Pact pact2(PactBuilder builder) {
    return builder
      .usingPlugin("csv")
      .expectsToReceive("request for to store a report", "core/interaction/http")
      .with(
        Map.of(
          "request.path", "/reports/report001.csv",
          "request.method", "POST",
          "request.contents", Map.of(
            "content-type", "text/csv",
            "column:1", "matching(type,'Name')",
            "column:2", "matching(number,100)",
            "column:3", "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
            ),
          "response.status", "201"
        )
      )
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "CsvServer", pactMethod = "pact")
  void getCsvReport(MockServer mockServer) throws IOException {
    CsvClient client = new CsvClient(mockServer.getUrl());
    String csv = client.fetch("report001.csv");
    String[] values = csv.trim().split(",");
    assertThat(values[0], is(equalTo("Name")));
    assertThat(values[1], is(equalTo("100")));
    assertThat(values[2], matchesRegex("\\d{4}-\\d{2}-\\d{2}"));
  }

  @Test
  @PactTestFor(providerName = "CsvServer", pactMethod = "pact2")
  void saveCsvReport(MockServer mockServer) throws IOException, InterruptedException {
    Faker faker = new Faker();
    Random random = new Random();
    CsvClient client = new CsvClient(mockServer.getUrl());
    List<String[]> data = new ArrayList<>();
    int rows = random.nextInt(100);
    for (int i = 0; i < rows; i++) {
        data.add(new String[]{
          faker.name().fullName(),
          String.valueOf(Math.abs(random.nextInt())),
          DateTimeFormatter.ISO_LOCAL_DATE.format(faker.date().birthday().toInstant().atOffset(ZoneOffset.UTC))
        });
    }
    assertThat(client.save("report001.csv", data), is(true));
  }
}
