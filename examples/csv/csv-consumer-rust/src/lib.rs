use anyhow::anyhow;
use reqwest::Url;

struct CsvClient {
  pub url: Url
}

impl CsvClient {
  pub fn new<S>(url: S) -> CsvClient where S: Into<Url> {
    CsvClient {
      url: url.into()
    }
  }

  pub async fn fetch(&self, report: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    client.get(self.url.join("/reports/")?.join(report)?).send().await?.text().await
      .map_err(|err| anyhow!("Request for report failed - {}", err))
  }

  pub async fn save(&self, report: &str, data: &Vec<String>) -> anyhow::Result<bool> {
    // StringBuilder csv = new StringBuilder();
    // for (String[] row: data) {
    // csv.append(String.join(",", row));
    // csv.append('\n');
    // }
    // StringEntity entity = new StringEntity(csv.toString(), ContentType.create("text/csv", "UTF-8"));
    // return Request.post(url + "/reports/" + report).body(entity).execute().returnResponse().getCode() == 201;
    todo!()
  }
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_models::prelude::*;
  use serde_json::json;

  use crate::CsvClient;

  #[tokio::test]
  async fn test_csv_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let csv_service = PactBuilder::new_v4("CsvClient", "CsvServer")
      .using_plugin("csv", None).await
      .interaction("request for a report", "core/interaction/http", |mut i| async move {
        i.request.path("/reports/report001.csv");
        i.response
          .ok()
          .contents(ContentType::from( "text/csv"), json!({
            "column:1": "matching(type,'Name')",
            "column:2": "matching(number,100)",
            "column:3": "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
          })).await;
        i.clone()
      })
      .await
      .start_mock_server();

    let client = CsvClient::new(csv_service.url().clone());
    let data = client.fetch("report001.csv").await;
    expect!(data).to(be_ok().value("Name,100,2000-01-01\n"));
  }
}
