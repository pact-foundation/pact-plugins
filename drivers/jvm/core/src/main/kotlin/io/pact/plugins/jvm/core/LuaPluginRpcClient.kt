package io.pact.plugins.jvm.core

import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import io.pact.plugins.jvm.core.Utils.mapToProtoStruct
import io.pact.plugins.jvm.core.Utils.structToMap
import io.pact.plugins.jvm.core.lua.LuaEngine
import java.nio.ByteBuffer

private const val NOT_A_TRANSPORT_PLUGIN =
  "is only supported by TRANSPORT plugins, not Lua content-matcher plugins"

/**
 * [PactPluginRpcClient] backed by a [LuaEngine] instead of a gRPC channel - each method
 * marshals a protobuf request message into plain Kotlin values (String/Double/Boolean/
 * ByteBuffer/Map/List), calls the corresponding Lua global function, and converts the
 * (also plain-Kotlin-shaped) return value back into the protobuf response message.
 */
class LuaPluginRpcClient(private val engine: LuaEngine) : PactPluginRpcClient {
  override fun initPlugin(request: PluginInitRequest): PluginInitResponse {
    val result = engine.callFunction("init", listOf(request.implementation, request.version))
    @Suppress("UNCHECKED_CAST")
    val items = result as? List<Any?> ?: emptyList()
    val entries = items.map { item ->
      @Suppress("UNCHECKED_CAST")
      val map = item as Map<String, Any?>
      val entryType = Plugin.CatalogueEntry.EntryType.valueOf(map["entryType"] as String)
      @Suppress("UNCHECKED_CAST")
      val values = (map["values"] as? Map<String, Any?>)?.mapValues { it.value.toString() } ?: emptyMap()
      Plugin.CatalogueEntry.newBuilder()
        .setType(entryType)
        .setKey(map["key"] as String)
        .putAllValues(values)
        .build()
    }
    return PluginInitResponse(entries)
  }

  override fun updateCatalogue(request: Plugin.Catalogue) {
    if (!engine.hasFunction("update_catalogue")) return
    val entries = request.catalogueList.map { entry ->
      mapOf("entryType" to entry.type.name, "key" to entry.key, "values" to entry.valuesMap)
    }
    engine.callFunction("update_catalogue", listOf(entries))
  }

  override fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse {
    val requestMap = mapOf(
      "expected" to bodyToLua(if (request.hasExpected()) request.expected else null),
      "actual" to bodyToLua(if (request.hasActual()) request.actual else null),
      "allow_unexpected_keys" to request.allowUnexpectedKeys,
      "rules" to matchingRulesToLua(request.rulesMap),
      "plugin_configuration" to pluginConfigurationToLua(
        if (request.hasPluginConfiguration()) request.pluginConfiguration else null
      )
    )
    val result = engine.callFunction("match_contents", listOf(requestMap))
    @Suppress("UNCHECKED_CAST")
    return luaToCompareResponse(result as? Map<String, Any?> ?: emptyMap())
  }

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse {
    val config: Any? = if (request.hasContentsConfig()) structToMap(request.contentsConfig) else null
    val result = engine.callFunction("configure_interaction", listOf(request.contentType, config))
    @Suppress("UNCHECKED_CAST")
    return luaToConfigureResponse(result as? Map<String, Any?> ?: emptyMap())
  }

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse {
    val builder = Plugin.GenerateContentResponse.newBuilder()
    if (!engine.hasFunction("generate_content")) {
      if (request.hasContents()) builder.contents = request.contents
      return builder.build()
    }
    val contents = bodyToLua(if (request.hasContents()) request.contents else null)
    val generators = request.generatorsMap.mapValues { (_, g) -> generatorToLua(g) }
    val testMode = when (request.testMode) {
      Plugin.GenerateContentRequest.TestMode.Consumer -> "Consumer"
      Plugin.GenerateContentRequest.TestMode.Provider -> "Provider"
      else -> "Unknown"
    }
    val result = engine.callFunction("generate_content", listOf(contents, generators, testMode))
    luaToBody(result)?.let { builder.contents = it }
    return builder.build()
  }

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse =
    throw UnsupportedOperationException("Mock servers $NOT_A_TRANSPORT_PLUGIN")

  override fun startMockServerV2(request: PluginV2.StartMockServerRequest): Plugin.StartMockServerResponse =
    throw UnsupportedOperationException("Mock servers $NOT_A_TRANSPORT_PLUGIN")

  override fun shutdownMockServer(request: Plugin.ShutdownMockServerRequest): Plugin.ShutdownMockServerResponse =
    throw UnsupportedOperationException("Mock servers $NOT_A_TRANSPORT_PLUGIN")

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults =
    throw UnsupportedOperationException("Mock servers $NOT_A_TRANSPORT_PLUGIN")

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    throw UnsupportedOperationException("prepareInteractionForVerification $NOT_A_TRANSPORT_PLUGIN")

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse =
    throw UnsupportedOperationException("verifyInteraction $NOT_A_TRANSPORT_PLUGIN")

  // ---- Body <-> Lua ----

  private fun bodyToLua(body: Plugin.Body?): Map<String, Any?>? {
    if (body == null) return null
    return mapOf(
      "content_type" to body.contentType,
      "contents" to if (body.hasContent()) ByteBuffer.wrap(body.content.value.toByteArray()) else null,
      "content_type_hint" to body.contentTypeHint.name
    )
  }

  private fun luaToBody(value: Any?): Plugin.Body? {
    if (value == null) return null
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?>
      ?: throw IllegalStateException("Expected a body table or nil from Lua, got $value")
    val builder = Plugin.Body.newBuilder()
    builder.contentType = map["content_type"] as? String ?: ""
    map["contents"]?.let { builder.content = BytesValue.of(ByteString.copyFrom(toByteArray(it))) }
    builder.contentTypeHint = when (map["content_type_hint"] as? String) {
      "TEXT" -> Plugin.Body.ContentTypeHint.TEXT
      "BINARY" -> Plugin.Body.ContentTypeHint.BINARY
      else -> Plugin.Body.ContentTypeHint.DEFAULT
    }
    return builder.build()
  }

  private fun toByteArray(value: Any): ByteArray = when (value) {
    is ByteBuffer -> {
      val duplicate = value.duplicate()
      ByteArray(duplicate.remaining()).also { duplicate.get(it) }
    }
    is String -> value.toByteArray(Charsets.UTF_8)
    else -> throw IllegalStateException("Expected string or byte buffer content from Lua, got $value")
  }

  // ---- Matching rules / generators / plugin configuration <-> Lua ----

  private fun matchingRulesToLua(rules: Map<String, Plugin.MatchingRules>): Map<String, Any?> =
    rules.mapValues { (_, ruleList) ->
      ruleList.ruleList.map { rule ->
        mapOf(
          "type" to rule.type,
          "values" to if (rule.hasValues()) structToMap(rule.values) else null
        )
      }
    }

  private fun generatorToLua(generator: Plugin.Generator): Map<String, Any?> = mapOf(
    "type" to generator.type,
    "values" to if (generator.hasValues()) structToMap(generator.values) else null
  )

  private fun pluginConfigurationToLua(config: Plugin.PluginConfiguration?): Map<String, Any?>? {
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

  private fun luaToPluginConfiguration(value: Any?): Plugin.PluginConfiguration? {
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

  private fun luaToCompareResponse(result: Map<String, Any?>): Plugin.CompareContentsResponse {
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

  private fun luaValueToContentMismatches(path: String, value: Any?): List<Plugin.ContentMismatch> {
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

  // ---- ConfigureInteraction <-> Lua ----

  private fun luaToConfigureResponse(result: Map<String, Any?>): Plugin.ConfigureInteractionResponse {
    val builder = Plugin.ConfigureInteractionResponse.newBuilder()
    @Suppress("UNCHECKED_CAST")
    val items = result["interactions"] as? List<Any?> ?: emptyList()
    for (item in items) {
      @Suppress("UNCHECKED_CAST")
      builder.addInteraction(luaToInteractionResponse(item as Map<String, Any?>))
    }
    result["plugin_config"]?.let { builder.pluginConfiguration = luaToPluginConfiguration(it) }
    return builder.build()
  }

  private fun luaToInteractionResponse(item: Map<String, Any?>): Plugin.InteractionResponse {
    val builder = Plugin.InteractionResponse.newBuilder()
    luaToBody(item["contents"])?.let { builder.contents = it }
    item["plugin_config"]?.let { builder.pluginConfiguration = luaToPluginConfiguration(it) }
    (item["part_name"] as? String)?.let { builder.partName = it }
    return builder.build()
  }
}
