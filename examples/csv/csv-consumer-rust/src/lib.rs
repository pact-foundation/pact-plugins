use anyhow::anyhow;
use csv::Writer;
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

  pub async fn save(&self, report: &str, data: &Vec<Vec<String>>) -> anyhow::Result<bool> {
    let mut wtr = Writer::from_writer(vec![]);
    for row in data {
      wtr.write_record(row)?;
    }

    let client = reqwest::Client::new();
    let response = client.post(self.url.join("/reports/")?.join(report)?)
      .header("content-type", "text/csv;charset=utf-8")
      .body(wtr.into_inner()?)
      .send()
      .await?;
    Ok(response.status().as_u16() == 201)
  }
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;
  use fakeit::{datetime, name};
  use pact_consumer::prelude::*;
  use pact_models::prelude::*;
  use rand::prelude::*;
  use regex::Regex;
  use serde_json::json;

  use crate::CsvClient;

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_csv_client() {
    let csv_service = PactBuilder::new_v4("CsvClient", "CsvServer")
      .using_plugin("csv", None).await
      .interaction("request for a report", "", |mut i| async move {
        i.request.path("/reports/report001.csv");
        i.response
          .ok()
          .contents(ContentType::from("text/csv"), json!({
            "csvHeaders": false,
            "column:1": "matching(type,'Name')",
            "column:2": "matching(number,100)",
            "column:3": "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
          })).await;
        i
      })
      .await
    .start_mock_server_async(None)
    .await;

    let client = CsvClient::new(csv_service.url().clone());
    let data = client.fetch("report001.csv").await.unwrap();

    let columns: Vec<&str> = data.trim().split(",").collect();
    expect!(columns.get(0)).to(be_some().value(&"Name"));
    expect!(columns.get(1)).to(be_some().value(&"100"));
    let date = columns.get(2).unwrap();
    let re = Regex::new("\\d{4}-\\d{2}-\\d{2}").unwrap();
    expect!(re.is_match(date)).to(be_true());
  }

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_post_csv() {
    let csv_service = PactBuilder::new_v4("CsvClient", "CsvServer")
      .using_plugin("csv", None).await
      .interaction("request for to store a report", "", |mut i| async move {
        i.request
          .path("/reports/report001.csv")
          .method("POST")
          .header("content-type", "text/csv")
          .contents(ContentType::from("text/csv"), json!({
            "csvHeaders": false,
            "column:1": "matching(type,'Name')",
            "column:2": "matching(number,100)",
            "column:3": "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
          })).await;

        i.response.created();

        i
      })
      .await
      .start_mock_server_async(None)
      .await;

    let client = CsvClient::new(csv_service.url().clone());
    let rows = random::<u8>();
    let mut data = vec![];
    for _ in 0..rows {
      let num: u8 = random();
      let month = datetime::month().parse::<u8>().unwrap_or_default();
      let day = datetime::day().parse::<u8>().unwrap_or_default();
      data.push(vec![
        name::full(),
        num.to_string(),
        format!("{}-{:02}-{:02}", datetime::year(), month, day)
      ]);
    }

    let result = client.save("report001.csv", &data).await;

    expect!(result).to(be_ok().value(true));
  }
}
