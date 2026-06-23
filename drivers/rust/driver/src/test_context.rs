//! Thread-local test run ID for log correlation across driver and plugin log entries

use std::cell::RefCell;

thread_local! {
  static CURRENT_TEST_RUN_ID: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Set the test run ID for the current thread. Pass `None` to clear it.
///
/// Should be called by the test framework (pact_consumer, pact_ffi) before any plugin
/// calls, so that the ID is included in `testContext` of outgoing gRPC requests and can
/// be used to correlate plugin log entries with a specific test.
pub fn set_test_run_id(id: Option<String>) {
  CURRENT_TEST_RUN_ID.with(|cell| *cell.borrow_mut() = id);
}

/// Return the test run ID set for the current thread, if any.
pub fn current_test_run_id() -> Option<String> {
  CURRENT_TEST_RUN_ID.with(|cell| cell.borrow().clone())
}

/// Build a `prost_types::Struct` containing `{ "testRunId": "<id>" }` when a test run ID
/// is set on the current thread, or return `None` if none is set.
pub(crate) fn current_test_context() -> Option<prost_types::Struct> {
  current_test_run_id().map(|id| {
    let mut fields = ::std::collections::BTreeMap::new();
    fields.insert(
      "testRunId".to_string(),
      prost_types::Value {
        kind: Some(prost_types::value::Kind::StringValue(id)),
      },
    );
    prost_types::Struct { fields }
  })
}
