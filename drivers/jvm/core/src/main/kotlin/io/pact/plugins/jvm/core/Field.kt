package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.matchingrules.MatchingRule
import au.com.dius.pact.core.support.Json.toJson
import au.com.dius.pact.core.support.json.JsonValue
import com.google.protobuf.ByteString
import com.google.protobuf.NullValue
import com.google.protobuf.Struct
import io.github.oshai.kotlinlogging.KotlinLogging
import io.pact.plugin.v2.PluginV2
import io.pact.plugins.jvm.core.Utils.jsonToValue
import io.pact.plugins.jvm.core.Utils.toProtoStruct
import io.pact.plugins.jvm.core.Utils.valueToJson

private val logger = KotlinLogging.logger {}

/**
 * Support for matching and generating individual field/element values.
 *
 * This is the field-level counterpart of [ContentMatcher]: where a content matcher owns a whole
 * content type, a field matcher applies one matching rule to one value inside somebody else's
 * content - a field in a JSON body, a header, a message metadata value. See proposal 006
 * (Field-level matchers and generators) for the design.
 *
 * The proto types used here are the V2 interface ones. Field-level operations were introduced in V2
 * and have no V1 equivalent, so a V1 plugin cannot provide them.
 */

/**
 * A single value being matched or generated at the field/element level.
 *
 * Each type Pact's matching rules discriminate is its own case, rather than everything sharing one
 * JSON-ish value type. A `google.protobuf.Value` carries a single number type (a double), which
 * would make `integer` and `decimal` indistinguishable and `type` wrong between a whole number and
 * a decimal - and those are exactly the rules whose job is to check a value's runtime type.
 */
sealed class FieldValue {
  /** A null value */
  object Null : FieldValue()

  /** A boolean */
  data class Bool(val value: Boolean) : FieldValue()

  /** A string */
  data class Text(val value: String) : FieldValue()

  /** A whole number */
  data class Integer(val value: Long) : FieldValue()

  /** A number with a fractional part */
  data class Decimal(val value: Double) : FieldValue()

  /** Raw bytes, for a value that is not representable as text */
  data class Binary(val value: ByteArray) : FieldValue() {
    override fun equals(other: Any?): Boolean {
      if (this === other) return true
      if (javaClass != other?.javaClass) return false
      return value.contentEquals((other as Binary).value)
    }

    override fun hashCode(): Int = value.contentHashCode()
  }

  /** A map or list, for a rule applied to a collection rather than a scalar */
  data class Structured(val value: JsonValue) : FieldValue()

  /** Convert to the protobuf form sent to a plugin or core handler */
  fun toProto(): PluginV2.FieldValue {
    val builder = PluginV2.FieldValue.newBuilder()
    when (this) {
      is Null -> builder.nullValue = NullValue.NULL_VALUE
      is Bool -> builder.booleanValue = value
      is Text -> builder.stringValue = value
      is Integer -> builder.integerValue = value
      is Decimal -> builder.decimalValue = value
      is Binary -> builder.binaryValue = ByteString.copyFrom(value)
      is Structured -> builder.structuredValue = jsonToValue(value)
    }
    return builder.build()
  }

  companion object {
    /**
     * Convert from the protobuf form returned by a plugin or core handler. An unset value is
     * treated as null, matching how an absent `oneof` reads everywhere else in the interface.
     */
    @JvmStatic
    fun fromProto(value: PluginV2.FieldValue): FieldValue {
      return when (value.valueCase) {
        PluginV2.FieldValue.ValueCase.BOOLEANVALUE -> Bool(value.booleanValue)
        PluginV2.FieldValue.ValueCase.STRINGVALUE -> Text(value.stringValue)
        PluginV2.FieldValue.ValueCase.INTEGERVALUE -> Integer(value.integerValue)
        PluginV2.FieldValue.ValueCase.DECIMALVALUE -> Decimal(value.decimalValue)
        PluginV2.FieldValue.ValueCase.BINARYVALUE -> Binary(value.binaryValue.toByteArray())
        PluginV2.FieldValue.ValueCase.STRUCTUREDVALUE -> Structured(valueToJson(value.structuredValue))
        else -> Null
      }
    }

    /**
     * Convert from a Pact JSON value, keeping whole numbers whole.
     */
    @JvmStatic
    fun fromJson(value: JsonValue): FieldValue {
      return when (value) {
        is JsonValue.Null -> Null
        is JsonValue.True -> Bool(true)
        is JsonValue.False -> Bool(false)
        is JsonValue.StringValue -> Text(value.asString()!!)
        is JsonValue.Integer -> Integer(value.toBigInteger().toLong())
        is JsonValue.Decimal -> Decimal(value.toBigDecimal().toDouble())
        else -> Structured(value)
      }
    }
  }
}

/**
 * Where a value sits and what is known about it, shared by matching and generation.
 */
data class FieldContext @JvmOverloads constructor(
  /** Path to the value, as a Pact matching rule expression (`$.card.number`) */
  val path: String,
  /**
   * Part of the interaction the value came from: `body`, `header`, `metadata`, `query`, `path`,
   * `status`. Only affects how a mismatch is reported; generation ignores it.
   */
  val category: String = "body",
  /** Plugin configuration persisted into the Pact file for this interaction */
  val pluginConfiguration: PluginConfiguration? = null,
  /** Context data provided by the test framework */
  val testContext: Map<String, JsonValue> = emptyMap()
)

/** Which side of the test a generator is running on */
enum class FieldTestMode {
  CONSUMER, PROVIDER, UNKNOWN;

  fun toProto(): PluginV2.GenerateContentRequest.TestMode = when (this) {
    CONSUMER -> PluginV2.GenerateContentRequest.TestMode.Consumer
    PROVIDER -> PluginV2.GenerateContentRequest.TestMode.Provider
    UNKNOWN -> PluginV2.GenerateContentRequest.TestMode.Unknown
  }
}

/**
 * Find the field-level matching rule with the given name. The name is resolved against the
 * catalogue the same way any other capability key is - see [CatalogueManager.resolveCapability] - so
 * `creditcard` finds a plugin's own rule and `type` finds the core `type` rule. Throws if the
 * name matches nothing, matches more than one rule, or names something that is not a matching rule.
 */
fun findFieldMatcher(name: String): FieldMatcher =
  FieldMatcher(CatalogueManager.resolveCapabilityEntry(name, CatalogueEntryType.MATCHER))

/**
 * Find the field-level generator with the given name. See [findFieldMatcher].
 */
fun findFieldGenerator(name: String): FieldGenerator =
  FieldGenerator(CatalogueManager.resolveCapabilityEntry(name, CatalogueEntryType.GENERATOR))

/**
 * Matching rule for a single field/element value, provided by a plugin or by a handler the host
 * framework registered (see [CoreFieldMatcher]).
 */
data class FieldMatcher(val catalogueEntry: CatalogueEntry) {
  /** If this is a matching rule provided by the core framework rather than a plugin */
  val isCore: Boolean
    get() = catalogueEntry.providerType == CatalogueEntryProviderType.CORE

  /** Catalogue entry key for this matching rule */
  val catalogueEntryKey: String
    get() = if (isCore) "core/matcher/${catalogueEntry.key}"
      else "plugin/${catalogueEntry.pluginName}/matcher/${catalogueEntry.key}"

  /** Name of the plugin that provides this matching rule */
  val pluginName: String
    get() = catalogueEntry.pluginName

  /**
   * Apply this matching rule to a single value.
   *
   * The context carries where the value lives and which part of the interaction it came from; both
   * are echoed back on any mismatch that does not place itself. An empty result means the value
   * matched.
   */
  fun matchField(
    rule: MatchingRule,
    expected: FieldValue,
    actual: FieldValue,
    context: FieldContext
  ): List<ContentMismatch> {
    val request = PluginV2.MatchFieldRequest.newBuilder()
      .setKey(catalogueEntry.key)
      .setRule(toProtoMatchingRule(rule))
      .setPath(context.path)
      .setMismatchType(context.category)
      .setExpected(expected.toProto())
      .setActual(actual.toProto())
      .setTestContext(toProtoStruct(withTestRunId(context.testContext)))
      .also { builder ->
        if (context.pluginConfiguration != null) {
          builder.pluginConfiguration = toProtoPluginConfiguration(context.pluginConfiguration)
        }
      }
      .build()

    val response = try {
      if (isCore) {
        val handler = CoreCapabilityRegistry.fieldMatcher(catalogueEntry.key)
          ?: throw PactCoreCapabilityNotFoundException(catalogueEntry.key)
        handler.matchField(request)
      } else {
        val plugin = DefaultPluginManager.lookupPlugin(pluginName, null)
          ?: throw PactPluginNotFoundException(pluginName, null)
        logger.debug { "Sending MatchField request to plugin $pluginName" }
        val chainId = CallChain.newCallChainId()
        val deadlineMs = CallChain.defaultDeadlineMs()
        plugin.withRpcClient { client -> client.matchFieldWithChain(request, chainId, deadlineMs) }
      }
    } catch (ex: Exception) {
      logger.error(ex) { "Field-level match call failed" }
      return listOf(mismatchFor(ex.message ?: ex.toString(), context))
    }

    return when {
      response.error.isNotEmpty() -> listOf(mismatchFor(response.error, context))
      else -> response.mismatchesList.map {
        ContentMismatch(
          expected = it.expected.toByteArray(),
          actual = it.actual.toByteArray(),
          mismatch = it.mismatch,
          // A mismatch that does not place itself is reported against the value being matched
          path = it.path.ifEmpty { context.path },
          diff = it.diff.ifEmpty { null },
          type = it.mismatchType.ifEmpty { context.category }
        )
      }
    }
  }
}

/**
 * Generator for a single field/element value. See [FieldMatcher].
 */
data class FieldGenerator(val catalogueEntry: CatalogueEntry) {
  /** If this is a generator provided by the core framework rather than a plugin */
  val isCore: Boolean
    get() = catalogueEntry.providerType == CatalogueEntryProviderType.CORE

  /** Catalogue entry key for this generator */
  val catalogueEntryKey: String
    get() = if (isCore) "core/generator/${catalogueEntry.key}"
      else "plugin/${catalogueEntry.pluginName}/generator/${catalogueEntry.key}"

  /** Name of the plugin that provides this generator */
  val pluginName: String
    get() = catalogueEntry.pluginName

  /**
   * Generate a single value, replacing the example value from the Pact interaction.
   */
  fun generateField(
    generator: Generator,
    example: FieldValue,
    mode: FieldTestMode,
    context: FieldContext
  ): FieldValue {
    val request = PluginV2.GenerateFieldRequest.newBuilder()
      .setKey(catalogueEntry.key)
      .setGenerator(toProtoGenerator(generator))
      .setPath(context.path)
      .setExampleValue(example.toProto())
      .setTestContext(toProtoStruct(withTestRunId(context.testContext)))
      .setTestMode(mode.toProto())
      .also { builder ->
        if (context.pluginConfiguration != null) {
          builder.pluginConfiguration = toProtoPluginConfiguration(context.pluginConfiguration)
        }
      }
      .build()

    val response = if (isCore) {
      val handler = CoreCapabilityRegistry.fieldGenerator(catalogueEntry.key)
        ?: throw PactCoreCapabilityNotFoundException(catalogueEntry.key)
      handler.generateField(request)
    } else {
      val plugin = DefaultPluginManager.lookupPlugin(pluginName, null)
        ?: throw PactPluginNotFoundException(pluginName, null)
      logger.debug { "Sending GenerateField request to plugin $pluginName" }
      val chainId = CallChain.newCallChainId()
      val deadlineMs = CallChain.defaultDeadlineMs()
      plugin.withRpcClient { client -> client.generateFieldWithChain(request, chainId, deadlineMs) }
    }

    if (response.error.isNotEmpty()) {
      throw PactFieldGenerationException(catalogueEntry.key, response.error)
    }
    if (!response.hasValue()) {
      throw PactFieldGenerationException(catalogueEntry.key, "the generator returned no value")
    }
    return FieldValue.fromProto(response.value)
  }
}

private fun mismatchFor(message: String, context: FieldContext) = ContentMismatch(
  expected = null,
  actual = null,
  mismatch = message,
  path = context.path,
  type = context.category
)

/**
 * The test context a field-level request carries, with the current test run ID added if the host
 * did not supply one.
 *
 * The host has no test context to hand at the point a matching rule is applied - it is deep inside
 * matching, several layers below anything that knows about the test - so without this the
 * `testContext` on a field request would always be empty and a plugin could not correlate what it
 * logs with the test that caused it. See proposals 006 and 008.
 */
private fun withTestRunId(testContext: Map<String, JsonValue>): Map<String, JsonValue> {
  val testRunId = TestContext.currentTestRunId()
  return if (testRunId != null && !testContext.containsKey("testRunId")) {
    testContext + ("testRunId" to JsonValue.StringValue(testRunId))
  } else {
    testContext
  }
}

private fun toProtoMatchingRule(rule: MatchingRule): PluginV2.MatchingRule {
  val builder = PluginV2.MatchingRule.newBuilder()
  return builder
    .setType(rule.name)
    .setValues(builder.valuesBuilder.putAllFields(rule.attributes.entries.associate {
      it.key to jsonToValue(it.value)
    }.toMutableMap()))
    .build()
}

private fun toProtoGenerator(generator: Generator): PluginV2.Generator {
  val values = Struct.newBuilder()
  generator.toMap(PactSpecVersion.V4).forEach { (key, value) ->
    values.putFields(key, jsonToValue(toJson(value)))
  }
  return PluginV2.Generator.newBuilder()
    .setType(generator.type)
    .setValues(values)
    .build()
}

private fun toProtoPluginConfiguration(config: PluginConfiguration): PluginV2.PluginConfiguration =
  PluginV2.PluginConfiguration.newBuilder()
    .setInteractionConfiguration(toProtoStruct(config.interactionConfiguration))
    .setPactConfiguration(toProtoStruct(config.pactConfiguration))
    .build()
