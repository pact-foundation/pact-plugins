#[cfg(test)]
mod tests {
  use expectest::prelude::*;
  use pact_consumer::prelude::*;
  use pact_models::prelude::*;
  use serde_json::json;

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
  }
}
