//! Module containing code to deal with verifying interactions via plugins

use std::collections::HashMap;

use bytes::Bytes;
use itertools::Either;
use pact_models::prelude::OptionalBody;
use serde_json::Value;

use crate::proto::{VerificationResult, verification_result_item};

/// Data required to execute the verification of an interaction
#[derive(Clone, Debug, Default)]
pub struct InteractionVerificationData {
  /// Data for the request of the interaction
  pub(crate) request_data: OptionalBody,
  /// Metadata associated with the request
  pub(crate) metadata: HashMap<String, Either<Value, Bytes>>
}


/// Result of running an integration verification
#[derive(Clone, Debug, Default)]
pub struct InteractionVerificationResult {
  /// If the verification was successful
  pub ok: bool,
  /// List of errors if not successful
  pub details: Vec<InteractionVerificationDetails>
}

/// Details on an individual failure
#[derive(Clone, Debug)]
pub enum InteractionVerificationDetails {
  /// Error occurred
  Error(String),

  /// Mismatch occurred
  Mismatch {
    expected: Bytes,
    actual: Bytes,
    mismatch: String,
    path: String
  }
}

impl From<&VerificationResult> for InteractionVerificationResult {
  fn from(result: &VerificationResult) -> Self {
    InteractionVerificationResult {
      ok: result.success,
      details: result.mismatches.iter()
        .filter_map(|r| r.result.as_ref().map(|r| match r {
          verification_result_item::Result::Error(err) => InteractionVerificationDetails::Error(err.to_string()),
          verification_result_item::Result::Mismatch(mismatch) => InteractionVerificationDetails::Mismatch {
            expected: mismatch.expected.clone().map(|b| Bytes::from(b)).unwrap_or_default(),
            actual: mismatch.actual.clone().map(|b| Bytes::from(b)).unwrap_or_default(),
            mismatch: mismatch.mismatch.to_string(),
            path: mismatch.path.to_string()
          }
        }))
        .collect()
    }
  }
}
