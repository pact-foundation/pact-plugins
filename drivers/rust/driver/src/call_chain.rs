//! Call-chain tracking for driver <-> plugin callback cycle detection and deadline propagation.
//!
//! When a plugin calls back into the driver's `PluginHost` service for a capability (see
//! [`crate::core_capabilities`] and proposal 007, Driver-plugin callback model), that call may
//! itself be forwarded to another plugin, which could in turn call back again. This module tracks
//! the chain of catalogue entry keys invoked under a single root call (identified by a
//! `pact-call-chain-id` gRPC metadata value) so a cycle - the same entry key being invoked twice
//! in the same chain - is rejected instead of deadlocking or recursing forever, and enforces an
//! absolute deadline (`pact-deadline-ms` gRPC metadata) so a callback can never outlive the
//! request that triggered it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lazy_static::lazy_static;
use uuid::Uuid;

/// gRPC metadata key carrying the call-chain ID on driver<->plugin callback requests.
pub const CALL_CHAIN_ID_METADATA_KEY: &str = "pact-call-chain-id";
/// gRPC metadata key carrying the absolute deadline (Unix epoch milliseconds) on driver<->plugin
/// callback requests.
pub const DEADLINE_METADATA_KEY: &str = "pact-deadline-ms";

/// Budget given to a new call chain started by the driver, and the fallback used when a callback
/// arrives with no deadline metadata (defensive default for a non-conforming plugin).
pub const DEFAULT_CALL_CHAIN_TIMEOUT: Duration = Duration::from_secs(30);

lazy_static! {
  static ref CALL_CHAINS: Mutex<HashMap<String, Vec<String>>> = Mutex::new(HashMap::new());
}

/// Generate a new call-chain ID for the root of a driver -> plugin call that may trigger
/// callbacks.
pub fn new_call_chain_id() -> String {
  Uuid::new_v4().to_string()
}

/// Current time as Unix epoch milliseconds.
pub fn now_ms() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Absolute deadline (Unix epoch milliseconds) `timeout` from now.
pub fn deadline_from(timeout: Duration) -> u64 {
  now_ms() + timeout.as_millis() as u64
}

/// Absolute deadline (Unix epoch milliseconds) `DEFAULT_CALL_CHAIN_TIMEOUT` from now, for
/// starting a new call chain.
pub fn default_deadline_ms() -> u64 {
  deadline_from(DEFAULT_CALL_CHAIN_TIMEOUT)
}

/// True if `deadline_ms` has already passed.
pub fn is_expired(deadline_ms: u64) -> bool {
  now_ms() >= deadline_ms
}

/// Remaining budget until `deadline_ms`, for use as the timeout of the next hop's own call.
/// Zero if the deadline has already passed.
pub fn remaining(deadline_ms: u64) -> Duration {
  Duration::from_millis(deadline_ms.saturating_sub(now_ms()))
}

/// Holds `entry_key`'s place on `chain_id`'s call stack until dropped, at which point it is
/// popped back off. Returned by [`push_call`].
#[derive(Debug)]
pub struct CallChainGuard {
  chain_id: String,
  entry_key: String,
}

impl Drop for CallChainGuard {
  fn drop(&mut self) {
    pop_call(&self.chain_id, &self.entry_key);
  }
}

/// Push `entry_key` onto `chain_id`'s call stack, detecting cycles.
///
/// Call this before dispatching a callback for `entry_key` under `chain_id`; hold onto the
/// returned guard for the duration of the dispatch so `entry_key` is popped back off when it
/// completes, however it completes. Returns `Err` describing the current chain if `entry_key` is
/// already on the stack - the same capability being invoked again before an earlier invocation of
/// it has returned, i.e. a cycle - and dispatch should not proceed.
pub fn push_call(chain_id: &str, entry_key: &str) -> Result<CallChainGuard, String> {
  let mut chains = CALL_CHAINS.lock().expect("CALL_CHAINS mutex poisoned");
  let stack = chains.entry(chain_id.to_string()).or_default();
  if stack.iter().any(|key| key == entry_key) {
    return Err(format!(
      "Cycle detected calling '{}': already in call chain {:?}", entry_key, stack
    ));
  }
  stack.push(entry_key.to_string());
  Ok(CallChainGuard { chain_id: chain_id.to_string(), entry_key: entry_key.to_string() })
}

fn pop_call(chain_id: &str, entry_key: &str) {
  let mut chains = CALL_CHAINS.lock().expect("CALL_CHAINS mutex poisoned");
  if let Some(stack) = chains.get_mut(chain_id) {
    if let Some(pos) = stack.iter().rposition(|key| key == entry_key) {
      stack.remove(pos);
    }
    if stack.is_empty() {
      chains.remove(chain_id);
    }
  }
}

#[cfg(test)]
mod tests {
  use expectest::prelude::*;

  use super::*;

  #[test]
  fn push_call_succeeds_for_a_new_entry_and_pops_on_drop() {
    let chain_id = "push_call_succeeds_for_a_new_entry_and_pops_on_drop";

    {
      let _guard = push_call(chain_id, "content-matcher/xml")
        .expect("expected the first push for a fresh chain to succeed");
      expect!(push_call(chain_id, "content-matcher/csv")).to(be_ok());
    }

    // both guards have dropped, so the chain should be gone and the keys reusable
    expect!(push_call(chain_id, "content-matcher/xml")).to(be_ok());
  }

  #[test]
  fn push_call_rejects_a_repeated_entry_key_in_the_same_chain() {
    let chain_id = "push_call_rejects_a_repeated_entry_key_in_the_same_chain";
    let _guard = push_call(chain_id, "content-matcher/xml").unwrap();

    let result = push_call(chain_id, "content-matcher/xml");

    expect!(result.is_err()).to(be_true());
    expect!(result.unwrap_err()).to(be_equal_to("Cycle detected calling 'content-matcher/xml': already in call chain [\"content-matcher/xml\"]".to_string()));
  }

  #[test]
  fn push_call_allows_the_same_entry_key_in_different_chains() {
    let key = "content-matcher/xml";
    let _guard_a = push_call("chain-a-push_call_allows_the_same_entry_key_in_different_chains", key).unwrap();

    expect!(push_call("chain-b-push_call_allows_the_same_entry_key_in_different_chains", key)).to(be_ok());
  }

  #[test]
  fn deadline_helpers_compute_expiry_and_remaining_budget() {
    let future_deadline = deadline_from(Duration::from_secs(60));
    expect!(is_expired(future_deadline)).to(be_false());
    expect!(remaining(future_deadline).as_secs() > 0).to(be_true());

    let past_deadline = now_ms().saturating_sub(1_000);
    expect!(is_expired(past_deadline)).to(be_true());
    expect!(remaining(past_deadline)).to(be_equal_to(Duration::ZERO));
  }
}
