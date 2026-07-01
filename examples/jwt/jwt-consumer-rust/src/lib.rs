#[cfg(test)]
mod tests {
  use std::time::{SystemTime, UNIX_EPOCH};

  use expectest::prelude::*;
  use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
  use pact_consumer::mock_server::StartMockServerAsync;
  use pact_consumer::prelude::*;
  use pact_models::prelude::ContentType;
  use serde::{Deserialize, Serialize};
  use serde_json::json;

  const PRIVATE_KEY: &str = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEoAIBAAKCAQEAxK9/t4VQGZgqj4QtPwdpBKD3RurJoc1oDplywEqjDn3E4oft
dpPQSi3Y8QO9+ZGWYw/84GSqS4f4NG+AULy1ayRfDG7tJ8D6BiU08QdjIa8qJU49
ONV5S0UVg7euxKTQLE0Bynfl5Th10z8UshcPif/RDeQT+hUyGaV4OBNb7eiQ4uJA
f3RoxG+v1nwbrdey/xunAgEmKaRXexEvKJkgvY4Qz5jAf6Ttv/77GwZutvFWyLWN
gtNBIG8Vw/HCrdbv4+BmZevQbIEqFotXsFdmKwkmVQblZLhIOqVlP5WGfcggHpS9
j+KrTnElxUEdz4s0GWyGZvb3oxkIMr/Esh9dfQIDAQABAoIBACp0oZJ5Pd2QeZtq
EiQ5UsNzhXzy4FxDTPNdzyXP1puhrVaitbDzXjIs7Fe2EZNmCDrQ8Cp1wEa2jm6v
JNkIqvZ6LuQtq5Z5st6RuHhQumbCe0v7M/7pIZoMSwUYKKr80ozFgJ32PJM6mUBk
rPB2Rt3ocPVZJrDEU4CytZ0RHLZhLLqnFtrnRs2nrQac8zizgm8LqfqsSJZLaRMl
VF57UfjkxkrTewT8A8JP3JQOEtpmVXWia5Pz2mNfL+tG8NCLQjvFbJOUjlK1+paM
ASO3jQC3yicFmCGzrJyN2fh8xq3ch5qPSD/Jr7WZf8QOlf4cKB4Xc0I8jXtZZ+K7
owfagucCgYEA6E/In0H/KzyJ/gnVm8mEQtI5mFS1VfnNX5yvfkhE5DyOqZjD2UlB
hmSEyOhPSA6Q6+1rqlsb2ZDhXtGdyNV6QydbK2R55AnMr+jHSiF/4CgK9rQSsK86
gl7/E6JpFYgcshn7TlHT+Vg+CbRfJilYWvtH246Y0f55Qfx5VRHKOasCgYEA2L29
B9sFP1LZIeRbZxjfoM7qfWJhrY7fa+24UWBP4o+pCPUgv22ixdLLHatXy0DoSY9U
1O8H5mTc6HV3PCsmx1+qS2jvhdjJiTdkM8hSTaCt4HXqlbx4kIPPiGT/gUeyVYqB
Xq7YFJc3mZcbIgJcUiHxNo3+xCs6Dzpv2EJyrXcCgYBebae/zGS7H2V75GV2aIgh
XTBaEfyPkPWA6sCO1TNjjpXyrAzXsqY2yX8L5xnq0TjpHV2JJnWAjxp8nznCm7uR
tlqhnbrKDY2s5zKymEFRTRV/yBxcwy1GNvT59yc9wFDhuBvlbu95x/uXmECg02d9
u+wue5z0prqFLunmwU9w9wJ/CwGXl86Hda+/VvlBqvqYYJIhVjyouSeIMPLhaUB6
zgZ9jvbjstTeby1FIzyQOMITCak9pZJ91DVLAoL0ixml3nn9K9coUqOvmEg3zmld
xJNkQQG7596qQKxw3XxDfU0mwTFHYIeAcYs8R5Bk0FVOWt1eYmbTiSKo0B0nkNPO
/wKBgFkJcbaMNqibbLNHKWj4yDzAIcq0ZGvj0WDbtUPbd4J4mqlDa35A1lgC8XBv
6+xx7JgmC/JwAPsthPeXTCbr0Or+NfC8cmujbrLzNdgIS32Ww+QxqvnzMap1ElOF
xGgMNbvImsticz5CUjXC1IkCnbdLrC16YFKMKguaRvTDDOF+
-----END RSA PRIVATE KEY-----"#;

  #[derive(Debug, Serialize, Deserialize)]
  struct Claims {
    aud: String,
    exp: u64,
    iss: String,
    sub: String,
  }

  #[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 1))]
  async fn test_post_jwt() {
    let mut builder = PactBuilder::new_v4("JwtClient", "JwtServer")
      .using_plugin("jwt", Some("0.0.0".to_string())).await;
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
    let jwt_service = builder.start_mock_server_async(None, None)
      .await;

    let client = reqwest::Client::builder().build().unwrap();

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let claims = Claims {
      aud: "1234566778".to_string(),
      exp: now,
      iss: "ldsdkdalds".to_string(),
      sub: "slksjkdjkdks".to_string(),
    };

    let encoding_key = EncodingKey::from_rsa_pem(PRIVATE_KEY.as_bytes()).unwrap();
    let header = Header {
      kid: Some("key-112345564".to_string()),
      ..Header::new(Algorithm::RS512)
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
