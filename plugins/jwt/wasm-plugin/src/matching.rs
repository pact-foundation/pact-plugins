use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use maplit::{hashmap, hashset};
use serde_json::{Map, Value};

use crate::ContentMismatch;
use crate::jwt::Token;
use crate::log;

pub(crate) fn validate_token(token: &Token, algorithm: &String, public_key: &String) -> Result<(), Vec<ContentMismatch>> {
  let mut mismatches = vec![];

  if let Err(signature_error) = crate::jwt::validate_signature(&token, algorithm, public_key) {
    mismatches.push(ContentMismatch {
      expected: vec![],
      actual: vec![],
      mismatch: format!("Actual token signature is not valid: {}", signature_error),
      path: "$".to_string(),
      diff: None,
      mismatch_type: "".to_string()
    });
  }

  let expiration_time = token.claims.get("exp")
    .cloned()
    .unwrap_or_default();
  if let Some(expiration_time) = expiration_time.as_u64() {
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or(Duration::new(0, 0))
      .as_secs();
    if expiration_time < now {
      mismatches.push(ContentMismatch {
        expected: vec![],
        actual: vec![],
        mismatch: format!("Actual token has expired (exp {} < current clock {})", expiration_time, now),
        path: "$.exp".to_string(),
        diff: None,
        mismatch_type: "".to_string()
      });
    }
  } else {
    mismatches.push(ContentMismatch {
      expected: vec![],
      actual: vec![],
      mismatch: "Actual token expiration time (exp) was missing or not a valid number".to_string(),
      path: "$.exp".to_string(),
      diff: None,
      mismatch_type: "".to_string()
    });
  }

  if let Some(not_before_time) = token.claims.get("nbf") {
    if let Some(not_before_time) = not_before_time.as_u64() {
      let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::new(0, 0))
        .as_secs();
      if not_before_time > now {
        mismatches.push(ContentMismatch {
          expected: vec![],
          actual: vec![],
          mismatch: format!("Actual token is not to be used yet (nbf {} > current clock {})", not_before_time, now),
          path: "$.exp".to_string(),
          diff: None,
          mismatch_type: "".to_string()
        });
      }
    } else {
      mismatches.push(ContentMismatch {
        expected: vec![],
        actual: vec![],
        mismatch: "Actual token not before time (nbf) is not a valid number".to_string(),
        path: "$.nbf".to_string(),
        diff: None,
        mismatch_type: "".to_string()
      });
    }
  }

  if mismatches.is_empty() {
    Ok(())
  } else {
    Err(mismatches)
  }
}

pub(crate) fn match_headers(expected: &Map<String, Value>, actual: &Map<String, Value>) -> Result<(), HashMap<String, Vec<ContentMismatch>>> {
  log("matching JWT headers");
  log(format!("expected headers: {:?}", expected).as_str());
  log(format!("actual headers: {:?}", actual).as_str());
  match_map(
    expected,
    actual,
    hashset!{"typ", "alg"},
    hashset!{"alg", "jku", "jwk", "kid", "x5u", "x5c", "x5t", "x5t#S256", "typ", "cty", "crit"},
    hashset!{"jku"}
  )
}

pub(crate) fn match_claims(expected: &Map<String, Value>, actual: &Map<String, Value>) -> Result<(), HashMap<String, Vec<ContentMismatch>>> {
  log("matching JWT claims");
  log(format!("expected claims: {:?}", expected).as_str());
  log(format!("actual claims: {:?}", actual).as_str());
  match_map(
    expected,
    actual,
    hashset!{"iss", "sub", "aud", "exp"},
    hashset!{},
    hashset!{"exp", "nbf", "iat", "jti"}
  )
}

fn match_map(
  expected: &Map<String, Value>,
  actual: &Map<String, Value>,
  compulsory_keys: HashSet<&str>,
  allow_keys: HashSet<&str>,
  keys_to_ignore: HashSet<&str>
) -> Result<(), HashMap<String, Vec<ContentMismatch>>> {
  let mut mismatches: HashMap<_, Vec<ContentMismatch>> = hashmap![];

  for (k, v) in expected {
    if !keys_to_ignore.contains(k.as_str()) {
      if let Some(actual_value) = actual.get(k) {
        if actual_value != v {
          mismatches
            .entry(k.clone())
            .or_default()
            .push(ContentMismatch {
              expected: v.to_string().as_bytes().to_vec(),
              actual: actual_value.to_string().as_bytes().to_vec(),
              mismatch: format!("Expected value {} but got value {}", v, actual_value),
              path: k.to_string(),
              diff: None,
              mismatch_type: "".to_string()
            })
        }
      } else {
        mismatches
          .entry(k.clone())
          .or_default()
          .push(ContentMismatch {
            expected: v.to_string().as_bytes().to_vec(),
            actual: vec![],
            mismatch: format!("Expected value {} but did not get a value", v),
            path: k.clone(),
            diff: None,
            mismatch_type: "".to_string()
          })
      }
    }
  }

  if !allow_keys.is_empty() {
    for (k, v) in actual {
      if !allow_keys.contains(k.as_str()) {
        mismatches
          .entry(k.clone())
          .or_default()
          .push(ContentMismatch {
            expected: vec![],
            actual: v.to_string().as_bytes().to_vec(),
            mismatch: format!("{} is not allowed as a key", k),
            path: k.clone(),
            diff: None,
            mismatch_type: "".to_string()
          })
      }
    }
  }

  for k in compulsory_keys {
    if !actual.contains_key(k) {
      mismatches
        .entry(k.to_string())
        .or_default()
        .push(ContentMismatch {
          expected: k.as_bytes().to_vec(),
          actual: vec![],
          mismatch: format!("{} is a compulsory key", k),
          path: k.to_string(),
          diff: None,
          mismatch_type: "".to_string()
        })
    }
  }

  if mismatches.is_empty() || mismatches.values().all(|v| v.is_empty()) {
    Ok(())
  } else {
    Err(mismatches)
  }
}
