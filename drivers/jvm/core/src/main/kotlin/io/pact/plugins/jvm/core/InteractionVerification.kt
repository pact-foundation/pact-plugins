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
  val ok: Boolean = false
)
