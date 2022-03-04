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
