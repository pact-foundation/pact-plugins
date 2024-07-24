use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE as BASE64;
use pact_models::generators::generate_hexadecimal;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::RsaPrivateKey;
use rsa::sha2::Sha512;
use rsa::signature::{SignatureEncoding, Signer};
use serde_json::{Map, Value};

use crate::json_to_string;
use crate::log;

pub(crate) fn build_header(config: &Map<String, Value>) -> anyhow::Result<Value> {
  let mut header = Map::new();
  if let Some(value) = config.get("algorithm") {
    header.insert("alg".to_string(), json_to_string(value).into());
  } else {
    header.insert("alg".to_string(), "RS512".into());
  };
  if let Some(value) = config.get("token-type") {
    header.insert("typ".to_string(), json_to_string(value).into());
  }
  if let Some(value) = config.get("key-id") {
    header.insert("kid".to_string(), json_to_string(value).into());
  }
  Ok(Value::Object(header))
}

pub(crate) fn build_claims(config: &Map<String, Value>) -> anyhow::Result<Value> {
  let mut claims = Map::new();
  claims.insert("jti".to_string(), generate_hexadecimal(16).into());
  claims.insert("iat".to_string(), SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().into());

  claims.insert("sub".to_string(),  config.get("subject")
    .cloned()
    .unwrap_or_else(|| format!("sub_{}", generate_hexadecimal(4)).into()));
  claims.insert("iss".to_string(),  config.get("issuer")
    .cloned()
    .unwrap_or_else(|| format!("iss_{}", generate_hexadecimal(4)).into()));
  claims.insert("aud".to_string(),  config.get("audience")
    .cloned()
    .unwrap_or_else(|| format!("aud_{}", generate_hexadecimal(4)).into()));

  // exp: now + expiryInMinutes * 60, // Current time + STS_TOKEN_EXPIRY_MINUTES minutes
  //     claims["exp"] = os.time() + 5 * 60
  let t = SystemTime::now()
    .duration_since(UNIX_EPOCH)?
    .as_secs()
    + 5 * 60;
  claims.insert("exp".to_string(), t.into());

  let excluded_keys = ["subject",
    "issuer",
    "audience",
    "token-type",
    "algorithm",
    "key-id",
    "private-key",
    "public-key",
   ];
  for (k, v) in config.iter()
    .filter(|(k, _v)| !excluded_keys.contains(&k.as_str())) {
    claims.insert(k.clone(), v.clone());
  }

  Ok(Value::Object(claims))
}

pub(crate) fn sign_token(
  _config: &Map<String, Value>,
  header: &Value,
  private_key: &str,
  base_token: &str
) -> anyhow::Result<String> {
  if header["alg"] != "RS512" {
    log(format!("Signature algorithm is set to {}", header["alg"]).as_str());
    log("Only the RS512 algorithm is supported at the moment");
    return Err(anyhow!("Only the RS512 algorithm is supported at the moment"));
  }

  let private_key = RsaPrivateKey::from_pkcs1_pem(private_key)?;
  let signing_key = SigningKey::<Sha512>::new(private_key);
  let signature = signing_key.try_sign(base_token.as_bytes())?;
  log(format!("Signature for token = [{}]", signature).as_str());
  let encoded_signature = BASE64.encode(signature.to_bytes());
  Ok(encoded_signature)
}
