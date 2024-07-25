use std::str::from_utf8;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64;
use pact_models::generators::generate_hexadecimal;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::sha2::Sha512;
use rsa::signature::{SignatureEncoding, Signer, Verifier};
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

#[derive(Debug, Clone)]
pub struct Token {
  pub header: Map<String, Value>,
  pub claims: Map<String, Value>,
  pub signature: String,
  pub encoded: String,
}

pub(crate) fn decode_token(token_bytes: &[u8]) -> anyhow::Result<Token> {
  let encoded_string = from_utf8(token_bytes)?;
  log(format!("Encoded token = {}", encoded_string).as_str());
  let parts: Vec<_> = encoded_string.split('.').collect();

  let header_part = parts.get(0)
    .ok_or_else(|| anyhow!("Token header was missing from token string"))?;
  let header = BASE64.decode(header_part)
    .map_err(|err| anyhow!("Failed to decode the token bytes: {}", err))
    .and_then(|bytes| serde_json::from_slice::<Value>(bytes.as_slice())
      .map_err(|err| anyhow!("Failed to parse token header as JSON: {}", err)))?;
  log(format!("Token header = {}", header).as_str());

  let claims_part = parts.get(1)
    .ok_or_else(|| anyhow!("Token claims was missing from token string"))?;
  let claims = BASE64.decode(claims_part)
    .map_err(|err| anyhow!("Failed to decode the token bytes: {}", err))
    .and_then(|bytes| serde_json::from_slice::<Value>(bytes.as_slice())
      .map_err(|err| anyhow!("Failed to parse token claims as JSON: {}", err)))?;
  log(format!("Token claims = {}", claims).as_str());

  let signature = parts.get(1)
    .ok_or_else(|| anyhow!("Token signature was missing from token string"))?;
  log(format!("Token signature = {}", signature).as_str());

  Ok(Token {
    header: header.as_object().cloned().unwrap_or_default(),
    claims: claims.as_object().cloned().unwrap_or_default(),
    signature: signature.to_string(),
    encoded: encoded_string.to_string()
  })
}

pub(crate) fn validate_signature(token: &Token, algorithm: &String, public_key: &String) -> anyhow::Result<()> {
  log(format!("Signature algorithm is set to {}", algorithm).as_str());
  if algorithm != "RS512" {
    return Err(anyhow!("Only the RS512 algorithm is supported at the moment"));
  }

  let public_key = RsaPublicKey::from_public_key_pem(public_key)?;
  let verifying_key = VerifyingKey::<Sha512>::new(public_key);
  let (base_token, sig) = token.encoded.rsplit_once('.')
    .ok_or_else(|| anyhow!("Encoded token is not valid, was expecting parts seperated with a '.'"))?;
  let signature = Signature::try_from(BASE64.decode(sig)?.as_slice())?;
  verifying_key.verify(base_token.as_bytes(), &signature)
    .map_err(|err| anyhow!("Failed to verify token signature: {}", err))
}
