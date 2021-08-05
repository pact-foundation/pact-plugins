package io.grpc.examples.csv;

import org.apache.hc.client5.http.fluent.Request;

import java.io.IOException;

public class CsvClient {
  private String url;

  public CsvClient(String url) {
    this.url = url;
  }

  public String fetch(String report) throws IOException {
    return Request.get(url + "/reports/" + report).execute().returnContent().asString();
  }
}
