package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.OptionalBody

/**
 * Data required to execute the verification of an interaction
 */
data class InteractionVerificationData(
  /**
   * Data for the request of the interaction
   */
  val requestData: OptionalBody,

  /**
   * Metadata associated with the request
   */
  val metadata: Map<String, Any?>
)

/**
 * Result of running an integration verification
 */
data class InteractionVerificationResult(
  val ok: Boolean = false,
  val details: List<InteractionVerificationDetails> = emptyList(),
  /** Output to display to the user */
  val output: List<String> = emptyList()
)

/**
 * Details on an individual failure
 */
sealed class InteractionVerificationDetails {
  /**
   * Error occurred
   */
  data class Error(val message: String) : InteractionVerificationDetails()

  /**
   * Mismatch occurred
   */
  data class Mismatch(
    val expected: Any?,
    val actual: Any?,
    val mismatch: String,
    val path: String
  ) : InteractionVerificationDetails()
}
