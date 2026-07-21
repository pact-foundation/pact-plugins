package io.pact.plugins.jvm.core

import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.Utils.mapToProtoStruct
import io.pact.plugins.jvm.core.Utils.structToMap
import java.nio.ByteBuffer

/**
 * Shared Body/MatchingRules/PluginConfiguration/CompareContents <-> Lua conversions, used by both
 * [LuaPluginRpcClient] (the driver calling into a plugin's own `match_contents`/`generate_content`)
 * and [LuaPactPlugin]'s `host_compare_contents`/`host_generate_content` host functions (a plugin
 * calling back into a host-provided or another plugin's capability - see proposal 007). Both
 * directions use the same Lua table shapes, so the same conversions apply either way.
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
  builder.testMode = when (testMode) {
    "Consumer" -> Plugin.GenerateContentRequest.TestMode.Consumer
    "Provider" -> Plugin.GenerateContentRequest.TestMode.Provider
    else -> Plugin.GenerateContentRequest.TestMode.Unknown
  }
  return builder.build()
}
