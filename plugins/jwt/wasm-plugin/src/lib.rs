use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE as BASE64;
use rsa::pkcs1::{DecodeRsaPrivateKey, LineEnding};
use rsa::pkcs8::EncodePublicKey;
use rsa::RsaPrivateKey;
use serde_json::{json, Value};

mod jwt;

wit_bindgen::generate!();

struct JwtPlugin {

}

export!(JwtPlugin);

impl Guest for JwtPlugin {
    fn init(implementation: String, version: String) -> Vec<CatalogueEntry> {
        log(format!("hello from the JWT plugin: {}, {}", implementation, version).as_str());

        vec![
            CatalogueEntry {
                entry_type: EntryType::ContentMatcher,
                key: "jwt".to_string(),
                values: vec![("content-types".to_string(), "application/jwt;application/jwt+json".to_string())]
            },
            CatalogueEntry {
                entry_type: EntryType::ContentGenerator,
                key: "jwt".to_string(),
                values: vec![("content-types".to_string(), "application/jwt;application/jwt+json".to_string())]
            }
        ]
    }

    fn update_catalogue(_catalogue: Vec<CatalogueEntry>) {
        // no-op
    }

    // Use to setup the data for an interaction. The config is the data supplied by the user test.
    // In this case, we use the data to create a JWT.
    fn configure_interaction(content_type: String, config: String) -> Result<InteractionDetails, String> {
        log(format!("Setting up interaction for {}", content_type).as_str());

        let config = serde_json::from_str::<Value>(config.as_str())
          .map_err(|err| format!("Failed to parse incoming JSON: {}", err))?
          .as_object()
          .cloned()
          .unwrap_or_default();

        if !config.contains_key("private-key") {
            return Err("No private-key given. An RSA private key is required to create a signed JWT".to_string())
        }

        let private_key = json_to_string(config.get("private-key").unwrap());
        let public_key = if let Some(key) = config.get("public-key") {
            json_to_string(key)
        } else {
            rsa_public_key(private_key.as_str())
              .map_err(|err| format!("Failed to extract public key from private key: {}", err))?
        };

        let header = jwt::build_header(&config)
          .map_err(|err| format!("Failed to build JWT header: {}", err))?;
        let payload = jwt::build_claims(&config)
          .map_err(|err| format!("Failed to build JWT claims: {}", err))?;

        let encoded_header = BASE64.encode(header.to_string());
        let encoded_payload = BASE64.encode(payload.to_string());
        let base_token = format!("{}.{}", encoded_header, encoded_payload);

        let signature = jwt::sign_token(&config, &header, private_key.as_str(), base_token.as_str())
          .map_err(|err| format!("Failed to sign JWT token: {}", err))?;
        let signed_token = format!("{}.{}", base_token, signature);

        let plugin_config = PluginConfiguration {
            interaction_configuration: json!({
                "public-key": public_key,
                "algorithm": format!("{}", header["alg"])
            }).to_string(),
            pact_configuration: "".to_string()
        };

        let contents = Body {
            content: signed_token.as_bytes().to_vec(),
            content_type: "application/jwt+json".to_string(),
            content_type_hint: Some(ContentTypeHint::Text)
        };
        let interaction_contents = InteractionContents {
            part_name: "".to_string(),
            contents,
            plugin_configuration: plugin_config.clone()
        };

        Ok(InteractionDetails {
            interaction: vec![interaction_contents],
            plugin_config: Some(plugin_config)
        })
    }
}

fn rsa_public_key(private_key: &str) -> anyhow::Result<String> {
    log("Decoding private key as PKCS8 format");
    let private_key = RsaPrivateKey::from_pkcs1_pem(private_key)?;
    let public_key = private_key.to_public_key();
    public_key.to_public_key_pem(LineEnding::LF)
      .map_err(|err| anyhow!("Failed to encode the public RSA key: {}", err))
}

fn json_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::default(),
        _ => value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use expectest::prelude::*;
    use super::*;

    #[test]
    fn it_works() {
        let result = JwtPlugin::init("".to_string(), "".to_string());
        expect!(result.len()).to(be_equal_to(2));
    }
}
