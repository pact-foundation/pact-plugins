package io.pact.plugins.jvm.core

/**
 * Configuration settings to use to start a mock server
 */
data class MockServerConfig(
  /**
   * Host interface to use for the mock server. Defaults to the loopback adapter (127.0.0.1).
   */
  val hostInterface: String = "",

  /**
   * Port number to bind to. Defaults to 0, which causes a random free port to be chosen.
   */
  val port: Int = 0,

  /**
   * If TLS should be used. If enabled, a mock server with a self-signed cert will be started (if the mock server
   * supports TLS).
   */
  val tls: Boolean = false
)

/**
 * Details on a running mock server
 */
data class MockServerDetails(
  /**
   * Unique key for the mock server
   */
  val key: String,

  /**
   * Base URL to the running mock server
   */
  val baseUrl: String,

  /**
   * Port the mock server is running on
   */
  val port: Int,

  /**
   * Plugin the mock server belongs to
   */
  val plugin: PactPlugin
)

/**
 * Results from the mock server. These will be returned when the mock server is shutdown
 */
data class MockServerResults(
  /** service + method that was requested */
  val path: String,
  /** If an error occurred trying to handle the request */
  val error: String?,
  /** Any mismatches that occurred */
  val mismatches: List<MockServerMismatch>
)

/**
 * Mismatch detected by the mock server
 */
data class MockServerMismatch(
  /** Expected data bytes */
  val expected: Any?,
  /** Actual data bytes */
  val actual: Any?,
  /** Description of the mismatch */
  val mismatch: String,
  /** Path to the item that was matched. This is the value as per the documented Pact matching rule expressions. */
  val path: String,
  /** Optional diff of the contents */
  val diff: String?
)
