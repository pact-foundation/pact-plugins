#[cfg(test)]
mod tests {
  use std::time::{SystemTime, UNIX_EPOCH};
  use expectest::prelude::*;
  use jsonwebtoken::{Algorithm, encode, EncodingKey, Header};
  use maplit::btreemap;
  use pact_consumer::mock_server::StartMockServerAsync;
  use pact_consumer::prelude::*;
  use pact_models::prelude::ContentType;
  use serde_json::json;
  use serde::{Deserialize, Serialize};

  const PRIVATE_KEY: &str = r#"-----BEGIN RSA PRIVATE KEY-----
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
-----END RSA PRIVATE KEY-----"#;

  #[derive(Debug, Serialize, Deserialize, Default)]
  struct Claims {
    aud: String,         // Optional. Audience
    exp: u64,            // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    iss: String,         // Optional. Issuer
    sub: String,         // Optional. Subject (whom token refers to)
  }

  #[test_log::test(tokio::test)]
  async fn test_post_jwt() {
    let mut builder = PactBuilder::new_v4("JwtClient", "JwtServer")
      .using_plugin("jwt", None).await;
    builder.interaction("request for a token exchange", "", |mut i| async move {
      i.request
        .path("/token")
        .method("POST")
        .contents(ContentType::from("application/jwt+json"), json!({
          "audience": "1234566778",
          "subject": "slksjkdjkdks",
          "issuer": "ldsdkdalds",
          "algorithm": "RS512",
          "key-id": "key-112345564",
          "private-key": PRIVATE_KEY
        })).await;
      i.response
        .ok()
        .contents(ContentType::from("application/jwt+json"), json!({
          "audience": "1234566778",
          "subject": "slksjkdjkdks",
          "issuer": "ldsdkdalds",
          "algorithm": "RS512",
          "key-id": "key-112345564",
          "private-key": PRIVATE_KEY
        })).await;
      i
    }).await;
    let jwt_service = builder.start_mock_server_async(None)
      .await;

    let client = reqwest::Client::builder().build().unwrap();

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let claims = Claims {
      aud: "1234566778".to_string(),
      exp: now,
      iss: "ldsdkdalds".to_string(),
      sub: "slksjkdjkdks".to_string(),
      .. Claims::default()
    };

    let encoding_key = EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes()).unwrap();
    let header = Header {
      kid: Some("key-112345564".to_string()),
      .. Header::new(Algorithm::RS512)
    };
    let token = encode(&header, &claims, &encoding_key).unwrap();

    let response = client.post(format!("http://127.0.0.1:{}/token", jwt_service.url().port().unwrap()))
      .header("content-type", "application/jwt+json")
      .body(token)
      .send()
      .await
      .unwrap();
    expect!(response.status().is_success()).to(be_true());
  }
}
