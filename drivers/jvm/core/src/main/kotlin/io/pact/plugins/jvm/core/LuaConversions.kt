package io.pact.plugins.jvm.core

import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.NullValue
import com.google.protobuf.Struct
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import io.pact.plugins.jvm.core.Utils.fromProtoValue
import io.pact.plugins.jvm.core.Utils.mapToProtoStruct
import io.pact.plugins.jvm.core.Utils.structToMap
import io.pact.plugins.jvm.core.Utils.toProtoValue
import java.nio.ByteBuffer

/**
 * Shared Body/MatchingRules/PluginConfiguration/CompareContents/field <-> Lua conversions, used by
 * both [LuaPluginRpcClient] (the driver calling into a plugin's own `match_contents`/
 * `generate_content`/`match_field`/`generate_field`) and [LuaPactPlugin]'s `host_*` host functions
 * (a plugin calling back into a host-provided or another plugin's capability - see proposals 006
 * and 007). Both directions use the same Lua table shapes, so the same conversions apply either
 * way.
 */

// ---- Body <-> Lua ----

internal fun bodyToLua(body: Plugin.Body?): Map<String, Any?>? {
  if (body == null) return null
  return mapOf(
    "content_type" to body.contentType,
    "contents" to if (body.hasContent()) ByteBuffer.wrap(body.content.value.toByteArray()) else null,
    "content_type_hint" to body.contentTypeHint.name
  )
}

internal fun luaToBody(value: Any?): Plugin.Body? {
  if (value == null) return null
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?>
    ?: throw IllegalStateException("Expected a body table or nil from Lua, got $value")
  val builder = Plugin.Body.newBuilder()
  builder.contentType = map["content_type"] as? String ?: ""
  map["contents"]?.let { builder.content = BytesValue.of(ByteString.copyFrom(luaContentToByteArray(it))) }
  builder.contentTypeHint = when (map["content_type_hint"] as? String) {
    "TEXT" -> Plugin.Body.ContentTypeHint.TEXT
    "BINARY" -> Plugin.Body.ContentTypeHint.BINARY
    else -> Plugin.Body.ContentTypeHint.DEFAULT
  }
  return builder.build()
}

internal fun luaContentToByteArray(value: Any): ByteArray = when (value) {
  is ByteBuffer -> {
    val duplicate = value.duplicate()
    ByteArray(duplicate.remaining()).also { duplicate.get(it) }
  }
  is String -> value.toByteArray(Charsets.UTF_8)
  else -> throw IllegalStateException("Expected string or byte buffer content from Lua, got $value")
}

// ---- Matching rules / generators / plugin configuration <-> Lua ----

internal fun matchingRulesToLua(rules: Map<String, Plugin.MatchingRules>): Map<String, Any?> =
  rules.mapValues { (_, ruleList) ->
    ruleList.ruleList.map { rule ->
      mapOf(
        "type" to rule.type,
        "values" to if (rule.hasValues()) structToMap(rule.values) else null
      )
    }
  }

/** Reverse of [matchingRulesToLua] - used by `host_compare_contents` to convert the rules a
 * plugin script builds when calling back into a host-provided or another plugin's matcher. */
internal fun luaToMatchingRules(value: Any?): Map<String, Plugin.MatchingRules> {
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?> ?: return emptyMap()
  return map.mapValues { (_, rulesValue) ->
    @Suppress("UNCHECKED_CAST")
    val rulesList = rulesValue as? List<Any?> ?: emptyList()
    val builder = Plugin.MatchingRules.newBuilder()
    for (ruleValue in rulesList) {
      @Suppress("UNCHECKED_CAST")
      val ruleMap = ruleValue as Map<String, Any?>
      val ruleBuilder = Plugin.MatchingRule.newBuilder().setType(ruleMap["type"] as String)
      @Suppress("UNCHECKED_CAST")
      (ruleMap["values"] as? Map<String, Any?>)?.let { ruleBuilder.values = mapToProtoStruct(it) }
      builder.addRule(ruleBuilder.build())
    }
    builder.build()
  }
}

internal fun generatorToLua(generator: Plugin.Generator): Map<String, Any?> = mapOf(
  "type" to generator.type,
  "values" to if (generator.hasValues()) structToMap(generator.values) else null
)

/** Reverse of [generatorToLua], applied to a whole generators map - used by
 * `host_generate_content`. */
internal fun luaToGenerators(value: Any?): Map<String, Plugin.Generator> {
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?> ?: return emptyMap()
  return map.mapValues { (_, generatorValue) ->
    @Suppress("UNCHECKED_CAST")
    val generatorMap = generatorValue as Map<String, Any?>
    val builder = Plugin.Generator.newBuilder().setType(generatorMap["type"] as String)
    @Suppress("UNCHECKED_CAST")
    (generatorMap["values"] as? Map<String, Any?>)?.let { builder.values = mapToProtoStruct(it) }
    builder.build()
  }
}

internal fun pluginConfigurationToLua(config: Plugin.PluginConfiguration?): Map<String, Any?>? {
  if (config == null) return null
  val map = mutableMapOf<String, Any?>()
  if (config.hasInteractionConfiguration()) {
    map["interaction_configuration"] = structToMap(config.interactionConfiguration)
  }
  if (config.hasPactConfiguration()) {
    map["pact_configuration"] = structToMap(config.pactConfiguration)
  }
  return map
}

internal fun luaToPluginConfiguration(value: Any?): Plugin.PluginConfiguration? {
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?> ?: return null
  val builder = Plugin.PluginConfiguration.newBuilder()
  @Suppress("UNCHECKED_CAST")
  (map["interaction_configuration"] as? Map<String, Any?>)?.let {
    builder.interactionConfiguration = mapToProtoStruct(it)
  }
  @Suppress("UNCHECKED_CAST")
  (map["pact_configuration"] as? Map<String, Any?>)?.let {
    builder.pactConfiguration = mapToProtoStruct(it)
  }
  return builder.build()
}

// ---- CompareContents <-> Lua ----

/** Reverse of the request map [LuaPluginRpcClient.compareContents] builds - the request table a
 * plugin script passes to `host_compare_contents(entryKey, request)` (see
 * [LuaPactPlugin.registerHostFunctions]) is the same shape its own `match_contents(request)`
 * function receives. */
internal fun luaToCompareRequest(map: Map<String, Any?>): Plugin.CompareContentsRequest {
  val builder = Plugin.CompareContentsRequest.newBuilder()
  luaToBody(map["expected"])?.let { builder.expected = it }
  luaToBody(map["actual"])?.let { builder.actual = it }
  (map["allow_unexpected_keys"] as? Boolean)?.let { builder.allowUnexpectedKeys = it }
  builder.putAllRules(luaToMatchingRules(map["rules"]))
  luaToPluginConfiguration(map["plugin_configuration"])?.let { builder.pluginConfiguration = it }
  return builder.build()
}

internal fun luaToCompareResponse(result: Map<String, Any?>): Plugin.CompareContentsResponse {
  val error = result["error"] as? String
  if (error != null) {
    return Plugin.CompareContentsResponse.newBuilder().setError(error).build()
  }

  @Suppress("UNCHECKED_CAST")
  val typeMismatch = result["type-mismatch"] as? Map<String, Any?>
  if (typeMismatch != null) {
    return Plugin.CompareContentsResponse.newBuilder()
      .setTypeMismatch(
        Plugin.ContentTypeMismatch.newBuilder()
          .setExpected(typeMismatch["expected"]?.toString() ?: "")
          .setActual(typeMismatch["actual"]?.toString() ?: "")
          .build()
      )
      .build()
  }

  @Suppress("UNCHECKED_CAST")
  val mismatches = result["mismatches"] as? Map<String, Any?> ?: emptyMap()
  val builder = Plugin.CompareContentsResponse.newBuilder()
  for ((path, value) in mismatches) {
    val list = luaValueToContentMismatches(path, value)
    if (list.isNotEmpty()) {
      builder.putResults(path, Plugin.ContentMismatches.newBuilder().addAllMismatches(list).build())
    }
  }
  return builder.build()
}

internal fun luaValueToContentMismatches(path: String, value: Any?): List<Plugin.ContentMismatch> {
  return when (value) {
    null -> emptyList()
    is List<*> -> value.flatMap { luaValueToContentMismatches(path, it) }
    is Map<*, *> -> {
      @Suppress("UNCHECKED_CAST")
      val map = value as Map<String, Any?>
      val mismatch = map["mismatch"] as? String
      if (mismatch != null) {
        val mismatchBuilder = Plugin.ContentMismatch.newBuilder()
          .setMismatch(mismatch)
          .setPath(map["path"] as? String ?: path)
        map["expected"]?.let { mismatchBuilder.expected = BytesValue.of(ByteString.copyFromUtf8(it.toString())) }
        map["actual"]?.let { mismatchBuilder.actual = BytesValue.of(ByteString.copyFromUtf8(it.toString())) }
        (map["diff"] as? String)?.let { mismatchBuilder.diff = it }
        (map["mismatch_type"] as? String)?.let { mismatchBuilder.mismatchType = it }
        listOf(mismatchBuilder.build())
      } else {
        emptyList()
      }
    }
    else -> listOf(
      Plugin.ContentMismatch.newBuilder().setMismatch(value.toString()).setPath(path).build()
    )
  }
}

/** Reverse of [luaToCompareResponse] - the map `host_compare_contents` returns is shaped exactly
 * like what a plugin's own `match_contents` function is expected to return, so a plugin can pass
 * a host/forwarded comparison's result straight through as its own response. */
internal fun compareResponseToLua(response: Plugin.CompareContentsResponse): Map<String, Any?> {
  if (response.error.isNotEmpty()) {
    return mapOf("error" to response.error)
  }
  if (response.hasTypeMismatch()) {
    return mapOf(
      "type-mismatch" to mapOf(
        "expected" to response.typeMismatch.expected,
        "actual" to response.typeMismatch.actual
      )
    )
  }
  if (response.resultsMap.isNotEmpty()) {
    return mapOf(
      "mismatches" to response.resultsMap.mapValues { (_, mismatches) ->
        mismatches.mismatchesList.map { contentMismatchToLua(it) }
      }
    )
  }
  return emptyMap()
}

/** Converts a single mismatch into the table shape [luaValueToContentMismatches] parses. */
private fun contentMismatchToLua(mismatch: Plugin.ContentMismatch): Map<String, Any?> {
  val map = mutableMapOf<String, Any?>("mismatch" to mismatch.mismatch, "path" to mismatch.path)
  if (mismatch.hasExpected()) map["expected"] = ByteBuffer.wrap(mismatch.expected.value.toByteArray())
  if (mismatch.hasActual()) map["actual"] = ByteBuffer.wrap(mismatch.actual.value.toByteArray())
  if (mismatch.diff.isNotEmpty()) map["diff"] = mismatch.diff
  if (mismatch.mismatchType.isNotEmpty()) map["mismatch_type"] = mismatch.mismatchType
  return map
}

// ---- GenerateContent <-> Lua ----

/** Converts the `(entryKey, contents, generators, testMode)` arguments a plugin script passes to
 * `host_generate_content` (see [LuaPactPlugin.registerHostFunctions]) into a
 * `GenerateContentRequest` - the same three trailing arguments its own
 * `generate_content(contents, generators, test_mode)` function receives. */
internal fun luaToGenerateRequest(contents: Any?, generators: Any?, testMode: String?): Plugin.GenerateContentRequest {
  val builder = Plugin.GenerateContentRequest.newBuilder()
  luaToBody(contents)?.let { builder.contents = it }
  builder.putAllGenerators(luaToGenerators(generators))
  builder.testMode = luaToTestMode(testMode)
  return builder.build()
}

/** The test mode name a Lua script sees. */
internal fun testModeToLua(testMode: Plugin.GenerateContentRequest.TestMode?): String = when (testMode) {
  Plugin.GenerateContentRequest.TestMode.Consumer -> "Consumer"
  Plugin.GenerateContentRequest.TestMode.Provider -> "Provider"
  else -> "Unknown"
}

/** Reverse of [testModeToLua]. Anything unrecognised (including a missing value) is `Unknown`,
 * rather than an error - the mode is context for the plugin, not a contract. */
internal fun luaToTestMode(testMode: String?): Plugin.GenerateContentRequest.TestMode = when (testMode) {
  "Consumer" -> Plugin.GenerateContentRequest.TestMode.Consumer
  "Provider" -> Plugin.GenerateContentRequest.TestMode.Provider
  else -> Plugin.GenerateContentRequest.TestMode.Unknown
}

// ---- MatchField / GenerateField <-> Lua ----
//
// The field-level messages exist only on the V2 interface (proposal 006), while the conversions
// above are written against the V1 message types. `PluginConfiguration` and `ContentMismatch` are
// identical between the two, so those conversions are reused through the small mappers below
// rather than being duplicated.

/**
 * Converts a single field value to a plain Lua value, following the convention message metadata
 * values already use (see `LuaPluginRpcClient.metadataToLua`): everything crosses as a plain value
 * except binary data, which arrives as a `{ binary = <string> }` wrapper so a script can tell a
 * string of text from a blob of bytes.
 *
 * A whole number crosses as a `Long`, which luajava pushes as a real Lua integer, so `math.type()`
 * in the script reports `"integer"`. That distinction is what the `integer`, `decimal` and `type`
 * rules are built on - see [FieldValue] and `LuaJavaEngine.readValue` for the other direction.
 */
internal fun fieldValueToLua(value: PluginV2.FieldValue?): Any? = when (value?.valueCase) {
  null, PluginV2.FieldValue.ValueCase.NULLVALUE, PluginV2.FieldValue.ValueCase.VALUE_NOT_SET -> null
  PluginV2.FieldValue.ValueCase.BOOLEANVALUE -> value.booleanValue
  PluginV2.FieldValue.ValueCase.STRINGVALUE -> value.stringValue
  PluginV2.FieldValue.ValueCase.INTEGERVALUE -> value.integerValue
  PluginV2.FieldValue.ValueCase.DECIMALVALUE -> value.decimalValue
  PluginV2.FieldValue.ValueCase.BINARYVALUE -> mapOf("binary" to ByteBuffer.wrap(value.binaryValue.toByteArray()))
  PluginV2.FieldValue.ValueCase.STRUCTUREDVALUE -> fromProtoValue(value.structuredValue)
}

/** Reverse of [fieldValueToLua]. A map is a binary wrapper if it has a `binary` key, and a
 * map or list otherwise - the same test `LuaPluginRpcClient.luaToMetadata` applies. */
internal fun luaToFieldValue(value: Any?): PluginV2.FieldValue {
  val builder = PluginV2.FieldValue.newBuilder()
  when (value) {
    null -> builder.nullValue = NullValue.NULL_VALUE
    is Boolean -> builder.booleanValue = value
    is String -> builder.stringValue = value
    // Long is what a Lua integer arrives as; Int/Short/Byte for a value the host built directly
    is Long, is Int, is Short, is Byte -> builder.integerValue = (value as Number).toLong()
    is Number -> builder.decimalValue = value.toDouble()
    is ByteBuffer -> builder.binaryValue = ByteString.copyFrom(luaContentToByteArray(value))
    is Map<*, *> -> {
      val binary = value["binary"]
      if (binary != null) {
        builder.binaryValue = ByteString.copyFrom(luaContentToByteArray(binary))
      } else {
        builder.structuredValue = toProtoValue(value)
      }
    }
    else -> builder.structuredValue = toProtoValue(value)
  }
  return builder.build()
}

/**
 * Converts a matching rule or a generator - each is a name plus optional configured values - into
 * the `{ type = "...", values = { ... } }` map a script already sees for the rules and generators
 * passed to `match_contents`/`generate_content`. Always a map, even with no values, so a script
 * can read `rule.values` without checking `rule` first.
 */
private fun typedValuesToLua(type: String, values: Struct?): Map<String, Any?> =
  mapOf("type" to type, "values" to values?.let { structToMap(it) })

internal fun matchingRuleToLua(rule: PluginV2.MatchingRule?): Map<String, Any?> =
  typedValuesToLua(rule?.type ?: "", if (rule != null && rule.hasValues()) rule.values else null)

internal fun fieldGeneratorToLua(generator: PluginV2.Generator?): Map<String, Any?> =
  typedValuesToLua(generator?.type ?: "", if (generator != null && generator.hasValues()) generator.values else null)

private fun luaToMatchingRule(value: Any?): PluginV2.MatchingRule {
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?>
    ?: throw IllegalStateException("Expected a 'rule' table from Lua, got $value")
  val builder = PluginV2.MatchingRule.newBuilder().setType(map["type"] as? String ?: "")
  @Suppress("UNCHECKED_CAST")
  (map["values"] as? Map<String, Any?>)?.let { builder.values = mapToProtoStruct(it) }
  return builder.build()
}

private fun luaToFieldGenerator(value: Any?): PluginV2.Generator {
  @Suppress("UNCHECKED_CAST")
  val map = value as? Map<String, Any?>
    ?: throw IllegalStateException("Expected a 'generator' table from Lua, got $value")
  val builder = PluginV2.Generator.newBuilder().setType(map["type"] as? String ?: "")
  @Suppress("UNCHECKED_CAST")
  (map["values"] as? Map<String, Any?>)?.let { builder.values = mapToProtoStruct(it) }
  return builder.build()
}

/** V2's `PluginConfiguration` carries the same two `Struct` fields as the V1 message
 * [pluginConfigurationToLua] converts, so the field-level requests reuse that conversion. */
internal fun v2PluginConfigurationToLua(config: PluginV2.PluginConfiguration?): Map<String, Any?>? {
  if (config == null) return null
  val builder = Plugin.PluginConfiguration.newBuilder()
  if (config.hasInteractionConfiguration()) builder.interactionConfiguration = config.interactionConfiguration
  if (config.hasPactConfiguration()) builder.pactConfiguration = config.pactConfiguration
  return pluginConfigurationToLua(builder.build())
}

/** Reverse of [v2PluginConfigurationToLua]. */
internal fun luaToV2PluginConfiguration(value: Any?): PluginV2.PluginConfiguration? {
  val config = luaToPluginConfiguration(value) ?: return null
  val builder = PluginV2.PluginConfiguration.newBuilder()
  if (config.hasInteractionConfiguration()) builder.interactionConfiguration = config.interactionConfiguration
  if (config.hasPactConfiguration()) builder.pactConfiguration = config.pactConfiguration
  return builder.build()
}

/** See [v2PluginConfigurationToLua] - `ContentMismatch` is likewise identical between the two
 * interfaces, so [luaValueToContentMismatches] is reused for the V2-only field messages. */
private fun v1ContentMismatchToV2(mismatch: Plugin.ContentMismatch): PluginV2.ContentMismatch {
  val builder = PluginV2.ContentMismatch.newBuilder()
    .setMismatch(mismatch.mismatch)
    .setPath(mismatch.path)
    .setDiff(mismatch.diff)
    .setMismatchType(mismatch.mismatchType)
  if (mismatch.hasExpected()) builder.expected = mismatch.expected
  if (mismatch.hasActual()) builder.actual = mismatch.actual
  return builder.build()
}

/** Reverse of [v1ContentMismatchToV2]. */
private fun v2ContentMismatchToV1(mismatch: PluginV2.ContentMismatch): Plugin.ContentMismatch {
  val builder = Plugin.ContentMismatch.newBuilder()
    .setMismatch(mismatch.mismatch)
    .setPath(mismatch.path)
    .setDiff(mismatch.diff)
    .setMismatchType(mismatch.mismatchType)
  if (mismatch.hasExpected()) builder.expected = mismatch.expected
  if (mismatch.hasActual()) builder.actual = mismatch.actual
  return builder.build()
}

/** Builds the request map a plugin's own `match_field(request)` function receives. */
internal fun matchFieldRequestToLua(request: PluginV2.MatchFieldRequest): Map<String, Any?> = mapOf(
  "key" to request.key,
  "rule" to matchingRuleToLua(if (request.hasRule()) request.rule else null),
  "path" to request.path,
  "mismatch_type" to request.mismatchType,
  "expected" to fieldValueToLua(if (request.hasExpected()) request.expected else null),
  "actual" to fieldValueToLua(if (request.hasActual()) request.actual else null),
  "plugin_configuration" to v2PluginConfigurationToLua(
    if (request.hasPluginConfiguration()) request.pluginConfiguration else null
  ),
  "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null
)

/** Reverse of [matchFieldRequestToLua] - the map a script builds when calling
 * `host_match_field(entry_key, request)` is the same shape its own `match_field` receives, so it
 * can forward the request it was given after adjusting whatever it needs to. */
internal fun luaToMatchFieldRequest(map: Map<String, Any?>): PluginV2.MatchFieldRequest {
  val builder = PluginV2.MatchFieldRequest.newBuilder()
    .setKey(map["key"] as? String ?: "")
    .setRule(luaToMatchingRule(map["rule"]))
    .setPath(map["path"] as? String ?: "")
    .setMismatchType(map["mismatch_type"] as? String ?: "")
    .setExpected(luaToFieldValue(map["expected"]))
    .setActual(luaToFieldValue(map["actual"]))
  luaToV2PluginConfiguration(map["plugin_configuration"])?.let { builder.pluginConfiguration = it }
  @Suppress("UNCHECKED_CAST")
  (map["test_context"] as? Map<String, Any?>)?.let { builder.testContext = mapToProtoStruct(it) }
  return builder.build()
}

/**
 * Parses the map a plugin's `match_field` function returns: `{ error = "..." }`, or
 * `{ mismatches = { ... } }` where each entry is a mismatch table or a bare description string
 * (the same leniency `match_contents` responses get - see [luaValueToContentMismatches]). An
 * absent or empty list means the value matched.
 */
internal fun luaToMatchFieldResponse(result: Map<String, Any?>, path: String): PluginV2.MatchFieldResponse {
  val error = result["error"] as? String
  if (error != null) {
    return PluginV2.MatchFieldResponse.newBuilder().setError(error).build()
  }
  return PluginV2.MatchFieldResponse.newBuilder()
    .addAllMismatches(luaValueToContentMismatches(path, result["mismatches"]).map { v1ContentMismatchToV2(it) })
    .build()
}

/** Reverse of [luaToMatchFieldResponse], so a script can return the result of a `host_match_field`
 * call straight through as its own response. */
internal fun matchFieldResponseToLua(response: PluginV2.MatchFieldResponse): Map<String, Any?> {
  if (response.error.isNotEmpty()) {
    return mapOf("error" to response.error)
  }
  return mapOf("mismatches" to response.mismatchesList.map { contentMismatchToLua(v2ContentMismatchToV1(it)) })
}

/** Builds the request map a plugin's own `generate_field(request)` function receives. */
internal fun generateFieldRequestToLua(request: PluginV2.GenerateFieldRequest): Map<String, Any?> = mapOf(
  "key" to request.key,
  "generator" to fieldGeneratorToLua(if (request.hasGenerator()) request.generator else null),
  "path" to request.path,
  "example_value" to fieldValueToLua(if (request.hasExampleValue()) request.exampleValue else null),
  "plugin_configuration" to v2PluginConfigurationToLua(
    if (request.hasPluginConfiguration()) request.pluginConfiguration else null
  ),
  "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null,
  "test_mode" to testModeToLua(
    Plugin.GenerateContentRequest.TestMode.forNumber(request.testModeValue)
  )
)

/** Reverse of [generateFieldRequestToLua] - the map a script passes to
 * `host_generate_field(entry_key, request)`. */
internal fun luaToGenerateFieldRequest(map: Map<String, Any?>): PluginV2.GenerateFieldRequest {
  val builder = PluginV2.GenerateFieldRequest.newBuilder()
    .setKey(map["key"] as? String ?: "")
    .setGenerator(luaToFieldGenerator(map["generator"]))
    .setPath(map["path"] as? String ?: "")
    .setExampleValue(luaToFieldValue(map["example_value"]))
    .setTestModeValue(luaToTestMode(map["test_mode"] as? String).number)
  luaToV2PluginConfiguration(map["plugin_configuration"])?.let { builder.pluginConfiguration = it }
  @Suppress("UNCHECKED_CAST")
  (map["test_context"] as? Map<String, Any?>)?.let { builder.testContext = mapToProtoStruct(it) }
  return builder.build()
}

/** Parses the map a plugin's `generate_field` function returns: `{ value = ... }` or
 * `{ error = "..." }`. */
internal fun luaToGenerateFieldResponse(result: Map<String, Any?>): PluginV2.GenerateFieldResponse {
  val error = result["error"] as? String
  if (error != null) {
    return PluginV2.GenerateFieldResponse.newBuilder().setError(error).build()
  }
  return PluginV2.GenerateFieldResponse.newBuilder().setValue(luaToFieldValue(result["value"])).build()
}

/** Reverse of [luaToGenerateFieldResponse], so a script can return the result of a
 * `host_generate_field` call straight through as its own response. */
internal fun generateFieldResponseToLua(response: PluginV2.GenerateFieldResponse): Map<String, Any?> {
  if (response.error.isNotEmpty()) {
    return mapOf("error" to response.error)
  }
  return mapOf("value" to fieldValueToLua(if (response.hasValue()) response.value else null))
}
