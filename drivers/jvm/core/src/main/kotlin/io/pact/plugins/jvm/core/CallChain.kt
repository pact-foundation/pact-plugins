package io.pact.plugins.jvm.core

import java.time.Duration
import java.util.UUID

/**
 * Call-chain tracking for driver <-> plugin callback cycle detection and deadline propagation.
 *
 * When a plugin calls back into the driver's `PluginHost` service for a capability (see
 * [CoreCapabilityRegistry] and proposal 007, Driver-plugin callback model), that call may itself
 * be forwarded to another plugin, which could in turn call back again. This object tracks the
 * chain of catalogue entry keys invoked under a single root call (identified by a
 * `pact-call-chain-id` gRPC metadata value) so a cycle - the same entry key being invoked twice in
 * the same chain - is rejected instead of deadlocking or recursing forever, and enforces an
 * absolute deadline (`pact-deadline-ms` gRPC metadata) so a callback can never outlive the request
 * that triggered it.
 *
 * This mirrors the Rust driver's `call_chain` module; see that module's doc comment for the same
 * design written out in full.
 */
object CallChain {
  /** gRPC metadata key carrying the call-chain ID on driver<->plugin callback requests. */
  const val CALL_CHAIN_ID_METADATA_KEY = "pact-call-chain-id"

  /**
   * gRPC metadata key carrying the absolute deadline (Unix epoch milliseconds) on driver<->plugin
   * callback requests.
   */
  const val DEADLINE_METADATA_KEY = "pact-deadline-ms"

  /**
   * Budget given to a new call chain started by the driver, and the fallback used when a callback
   * arrives with no deadline metadata (defensive default for a non-conforming plugin).
   */
  val DEFAULT_CALL_CHAIN_TIMEOUT: Duration = Duration.ofSeconds(30)

  private val chains = mutableMapOf<String, MutableList<String>>()

  /**
   * Generate a new call-chain ID for the root of a driver -> plugin call that may trigger
   * callbacks.
   */
  fun newCallChainId(): String = UUID.randomUUID().toString()

  /** Current time as Unix epoch milliseconds. */
  fun nowMs(): Long = System.currentTimeMillis()

  /** Absolute deadline (Unix epoch milliseconds) `timeout` from now. */
  fun deadlineFrom(timeout: Duration): Long = nowMs() + timeout.toMillis()

  /**
   * Absolute deadline (Unix epoch milliseconds) [DEFAULT_CALL_CHAIN_TIMEOUT] from now, for
   * starting a new call chain.
   */
  fun defaultDeadlineMs(): Long = deadlineFrom(DEFAULT_CALL_CHAIN_TIMEOUT)

  /** True if `deadlineMs` has already passed. */
  fun isExpired(deadlineMs: Long): Boolean = nowMs() >= deadlineMs

  /**
   * Remaining budget until `deadlineMs`, for use as the timeout of the next hop's own call. Zero
   * if the deadline has already passed.
   */
  fun remaining(deadlineMs: Long): Duration = Duration.ofMillis(maxOf(0L, deadlineMs - nowMs()))

  /**
   * Holds `entryKey`'s place on `chainId`'s call stack until closed, at which point it is popped
   * back off. Returned by [pushCall]; use it in a `.use { }` block so it is always released.
   */
  class CallChainGuard internal constructor(private val chainId: String, private val entryKey: String) : AutoCloseable {
    override fun close() {
      popCall(chainId, entryKey)
    }
  }

  /**
   * Push `entryKey` onto `chainId`'s call stack, detecting cycles.
   *
   * Call this before dispatching a callback for `entryKey` under `chainId`; use the returned
   * guard in a `.use { }` block so `entryKey` is popped back off when it completes, however it
   * completes. Throws [PactCallChainCycleException] if `entryKey` is already on the stack - the
   * same capability being invoked again before an earlier invocation of it has returned, i.e. a
   * cycle - and dispatch should not proceed.
   */
  @Synchronized
  fun pushCall(chainId: String, entryKey: String): CallChainGuard {
    val stack = chains.getOrPut(chainId) { mutableListOf() }
    if (stack.contains(entryKey)) {
      throw PactCallChainCycleException(entryKey, stack.toList())
    }
    stack.add(entryKey)
    return CallChainGuard(chainId, entryKey)
  }

  @Synchronized
  private fun popCall(chainId: String, entryKey: String) {
    val stack = chains[chainId] ?: return
    stack.remove(entryKey)
    if (stack.isEmpty()) {
      chains.remove(chainId)
    }
  }
}
