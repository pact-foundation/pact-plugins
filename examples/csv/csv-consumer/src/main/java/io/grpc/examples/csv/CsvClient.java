package io.grpc.examples.csv;

import org.apache.hc.client5.http.fluent.Request;
import org.apache.hc.core5.http.ContentType;
import org.apache.hc.core5.http.io.entity.StringEntity;

import java.io.IOException;
import java.util.List;

public class CsvClient {
  private String url;

  public CsvClient(String url) {
    this.url = url;
  }

  public String fetch(String report) throws IOException {
    return Request.get(url + "/reports/" + report).execute().returnContent().asString();
  }

  public boolean save(String report, List<String[]> data) throws IOException {
    StringBuilder csv = new StringBuilder();
    for (String[] row: data) {
      csv.append(String.join(",", row));
      csv.append('\n');
    }
    StringEntity entity = new StringEntity(csv.toString(), ContentType.create("text/csv", "UTF-8"));
    return Request.post(url + "/reports/" + report).body(entity).execute().returnResponse().getCode() == 201;
  }
}
