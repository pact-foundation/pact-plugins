package io.pact.plugins.jvm.core

/**
 * Thread-local storage for the current test run ID.
 *
 * Set a UUID before making plugin calls so the ID is included in `testContext` of outgoing
 * gRPC requests and can be used to correlate plugin log entries with a specific test run.
 */
object TestContext {
  private val currentTestRunId: ThreadLocal<String?> = ThreadLocal()

  /** Set the test run ID for the current thread. Pass null to clear it. */
  @JvmStatic
  fun setTestRunId(id: String?) {
    currentTestRunId.set(id)
  }

  /** Return the test run ID set for the current thread, or null if not set. */
  @JvmStatic
  fun currentTestRunId(): String? = currentTestRunId.get()
}
