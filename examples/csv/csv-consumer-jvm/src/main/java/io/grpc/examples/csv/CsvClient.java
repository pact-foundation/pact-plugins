package io.grpc.examples.csv;

import org.apache.commons.csv.CSVFormat;
import org.apache.commons.csv.CSVParser;
import org.apache.commons.csv.CSVRecord;
import org.apache.hc.client5.http.fluent.Request;
import org.apache.hc.core5.http.ContentType;
import org.apache.hc.core5.http.io.entity.StringEntity;

import java.io.IOException;
import java.io.StringReader;
import java.util.List;

public class CsvClient {
  private String url;

  public CsvClient(String url) {
    this.url = url;
  }

  public List<CSVRecord> fetch(String report, boolean hasHeaders) throws IOException {
    String contents = Request.get(url + "/reports/" + report).execute().returnContent().asString();
    CSVParser parser;
    if (hasHeaders) {
      parser = CSVFormat.EXCEL.builder()
        .setHeader()
        .setSkipHeaderRecord(true)
        .build()
        .parse(new StringReader(contents));
    } else {
      parser = CSVParser.parse(contents, CSVFormat.EXCEL);
    }
    return parser.getRecords();
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
