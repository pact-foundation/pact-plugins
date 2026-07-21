package io.pact.plugins.jvm.core

class PactPluginNotFoundException(val name: String, val version: String?) :
    RuntimeException("Plugin $name with version ${version ?: "any"} was not found")

class PactPluginEntryNotFoundException(val type: String) :
  RuntimeException("No interaction type of '$type' was found in the catalogue")

class PactPluginMockServerErrorException(val name: String, val error: String) :
  RuntimeException("Plugin $name failed to start a mock server: $error")

class PactPluginValidationForInteractionException(val name: String, val error: String) :
  RuntimeException("Plugin $name failed to validate the interaction: $error")

class PactPluginInteractionVerificationException(val name: String, val error: String) :
  RuntimeException("Plugin $name failed to run the verification for the interaction: $error")

class PactCoreCapabilityNotFoundException(val key: String) :
  RuntimeException("No core capability handler registered for '$key'")

class PactCatalogueEntryNotFoundException(val key: String) :
  RuntimeException("No catalogue entry found for key '$key'")

class PactCatalogueEntryTypeMismatchException(val key: String, val actualType: CatalogueEntryType, val expectedType: CatalogueEntryType) :
  RuntimeException("Catalogue entry '$key' is a $actualType, not a $expectedType")

class PactCallChainCycleException(val entryKey: String, val chain: List<String>) :
  RuntimeException("Cycle detected calling '$entryKey': already in call chain $chain")

class PactCallChainDeadlineExceededException(val chainId: String) :
  RuntimeException("Call chain $chainId deadline has already passed")
