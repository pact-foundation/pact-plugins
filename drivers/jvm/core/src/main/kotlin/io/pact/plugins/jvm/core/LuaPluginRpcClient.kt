package io.pact.plugins.jvm.core

import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import io.pact.plugins.jvm.core.Utils.fromProtoValue
import io.pact.plugins.jvm.core.Utils.structToMap
import io.pact.plugins.jvm.core.Utils.toProtoValue
import io.pact.plugins.jvm.core.lua.LuaEngine
import java.nio.ByteBuffer

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

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse {
    val requestMap = mapOf(
      "host_interface" to request.hostInterface,
      "port" to request.port,
      "tls" to request.tls,
      "pact" to request.pact,
      "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null
    )
    val result = engine.callFunction("start_mock_server", listOf(requestMap))
    return luaToStartMockServerResponse(result)
  }

  override fun startMockServerV2(request: PluginV2.StartMockServerRequest): Plugin.StartMockServerResponse {
    val requestMap = mapOf(
      "host_interface" to request.hostInterface,
      "port" to request.port,
      "tls" to request.tls,
      "interactions" to request.interactionsList.map { interactionContentsToLua(it) },
      "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null
    )
    val result = engine.callFunction("start_mock_server", listOf(requestMap))
    return luaToStartMockServerResponse(result)
  }

  override fun shutdownMockServer(request: Plugin.ShutdownMockServerRequest): Plugin.ShutdownMockServerResponse {
    val result = engine.callFunction("shutdown_mock_server", listOf(request.serverKey))
    val results = luaToMockServerResults(result)
    return Plugin.ShutdownMockServerResponse.newBuilder()
      .setOk(results.ok)
      .addAllResults(results.resultsList)
      .build()
  }

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults {
    val result = engine.callFunction("get_mock_server_results", listOf(request.serverKey))
    return luaToMockServerResults(result)
  }

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse {
    val requestMap = mapOf(
      "pact" to request.pact,
      "interaction_key" to request.interactionKey,
      "config" to if (request.hasConfig()) structToMap(request.config) else null
    )
    val result = engine.callFunction("prepare_interaction_for_verification", listOf(requestMap))
    return luaToVerificationPreparationResponse(result)
  }

  override fun prepareInteractionForVerificationV2(
    request: PluginV2.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse {
    val requestMap = mutableMapOf<String, Any?>(
      "config" to if (request.hasConfig()) structToMap(request.config) else null,
      "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null
    )
    if (request.hasInteractionContents()) {
      requestMap["interaction_contents"] = interactionContentsToLua(request.interactionContents)
    }
    val result = engine.callFunction("prepare_interaction_for_verification", listOf(requestMap))
    return luaToVerificationPreparationResponse(result)
  }

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse {
    val requestMap = mapOf(
      "interaction_data" to interactionDataToLua(if (request.hasInteractionData()) request.interactionData else null),
      "config" to if (request.hasConfig()) structToMap(request.config) else null,
      "pact" to request.pact,
      "interaction_key" to request.interactionKey
    )
    val result = engine.callFunction("verify_interaction", listOf(requestMap))
    return luaToVerifyInteractionResponse(result)
  }

  override fun verifyInteractionV2(request: PluginV2.VerifyInteractionRequest): Plugin.VerifyInteractionResponse {
    val interactionDataV1 = if (request.hasInteractionData()) {
      Plugin.InteractionData.parser().parseFrom(request.interactionData.toByteArray())
    } else {
      null
    }
    val requestMap = mutableMapOf<String, Any?>(
      "interaction_data" to interactionDataToLua(interactionDataV1),
      "config" to if (request.hasConfig()) structToMap(request.config) else null,
      "test_context" to if (request.hasTestContext()) structToMap(request.testContext) else null
    )
    if (request.hasInteractionContents()) {
      requestMap["interaction_contents"] = interactionContentsToLua(request.interactionContents)
    }
    val result = engine.callFunction("verify_interaction", listOf(requestMap))
    return luaToVerifyInteractionResponse(result)
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

  // ---- TRANSPORT plugin support: mock server / verification <-> Lua ----

  /**
   * Converts V2 `InteractionContents` (structured per-interaction data sent in place of a whole
   * Pact JSON document) into a Lua-facing map shaped as
   * `{ interaction_type, consumer, provider, plugin_configuration = { interaction_configuration, pact_configuration } }`.
   */
  private fun interactionContentsToLua(contents: PluginV2.InteractionContents): Map<String, Any?> {
    val map = mutableMapOf<String, Any?>(
      "interaction_type" to contents.interactionType,
      "consumer" to contents.consumer,
      "provider" to contents.provider
    )
    if (contents.hasPluginConfiguration()) {
      val pluginConfiguration = contents.pluginConfiguration
      val configMap = mutableMapOf<String, Any?>()
      if (pluginConfiguration.hasInteractionConfiguration()) {
        configMap["interaction_configuration"] = structToMap(pluginConfiguration.interactionConfiguration)
      }
      if (pluginConfiguration.hasPactConfiguration()) {
        configMap["pact_configuration"] = structToMap(pluginConfiguration.pactConfiguration)
      }
      map["plugin_configuration"] = configMap
    }
    return map
  }

  /**
   * Converts request/response metadata to a Lua-facing map. Each value is either a plain
   * value (JSON-like, for a non-binary `MetadataValue`) or a `{ "binary" -> ByteBuffer }`
   * wrapper map (for a binary `MetadataValue`), so a Lua script can tell the two apart.
   */
  private fun metadataToLua(metadata: Map<String, Plugin.MetadataValue>): Map<String, Any?> =
    metadata.mapValues { (_, value) ->
      when (value.valueCase) {
        Plugin.MetadataValue.ValueCase.NONBINARYVALUE -> fromProtoValue(value.nonBinaryValue)
        Plugin.MetadataValue.ValueCase.BINARYVALUE -> mapOf("binary" to ByteBuffer.wrap(value.binaryValue.toByteArray()))
        else -> null
      }
    }

  /** Converts a Lua metadata map (see [metadataToLua]) back into `MetadataValue`s. */
  private fun luaToMetadata(value: Any?): Map<String, Plugin.MetadataValue> {
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?> ?: return emptyMap()
    return map.mapValues { (_, entryValue) ->
      @Suppress("UNCHECKED_CAST")
      val wrapper = entryValue as? Map<String, Any?>
      val binary = wrapper?.get("binary")
      val builder = Plugin.MetadataValue.newBuilder()
      if (binary != null) {
        builder.binaryValue = ByteString.copyFrom(luaContentToByteArray(binary))
      } else {
        builder.nonBinaryValue = toProtoValue(entryValue)
      }
      builder.build()
    }
  }

  /**
   * Converts `InteractionData` (a request/response body plus metadata) to a Lua-facing map
   * shaped as `{ body = <body map>, metadata = <metadata map> }`, or `null` if not set.
   */
  private fun interactionDataToLua(data: Plugin.InteractionData?): Map<String, Any?>? {
    if (data == null) return null
    return mapOf(
      "body" to bodyToLua(if (data.hasBody()) data.body else null),
      "metadata" to metadataToLua(data.metadataMap)
    )
  }

  /** Converts a Lua interaction-data map (see [interactionDataToLua]) back into `InteractionData`. */
  private fun luaToInteractionData(value: Any?): Plugin.InteractionData? {
    if (value == null) return null
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?>
      ?: throw IllegalStateException("Expected an interaction data table or nil from Lua, got $value")
    val builder = Plugin.InteractionData.newBuilder()
    luaToBody(map["body"])?.let { builder.body = it }
    builder.putAllMetadata(luaToMetadata(map["metadata"]))
    return builder.build()
  }

  /**
   * Converts the map returned by the Lua `start_mock_server` function, shaped as either
   * `{ error = "..." }` or `{ details = { key, port, address } }`, into a `StartMockServerResponse`.
   */
  private fun luaToStartMockServerResponse(value: Any?): Plugin.StartMockServerResponse {
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?> ?: emptyMap()
    val error = map["error"] as? String
    if (error != null) {
      return Plugin.StartMockServerResponse.newBuilder().setError(error).build()
    }
    @Suppress("UNCHECKED_CAST")
    val details = map["details"] as? Map<String, Any?>
      ?: throw IllegalStateException("Lua start_mock_server() must return either 'error' or 'details'")
    return Plugin.StartMockServerResponse.newBuilder()
      .setDetails(
        Plugin.MockServerDetails.newBuilder()
          .setKey(details["key"] as String)
          .setPort((details["port"] as Number).toInt())
          .setAddress(details["address"] as String)
          .build()
      )
      .build()
  }

  /**
   * Converts the map returned by the Lua `shutdown_mock_server`/`get_mock_server_results`
   * functions, shaped as `{ ok = bool, results = { { path, error, mismatches = { ... } }, ... } }`,
   * into `MockServerResults`. Reuses [luaValueToContentMismatches] for each result's
   * `mismatches` field, the same helper `match_contents` responses use.
   */
  private fun luaToMockServerResults(value: Any?): Plugin.MockServerResults {
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?> ?: emptyMap()
    val ok = map["ok"] as? Boolean ?: true
    @Suppress("UNCHECKED_CAST")
    val resultsList = map["results"] as? List<Any?> ?: emptyList()
    val builder = Plugin.MockServerResults.newBuilder().setOk(ok)
    for (item in resultsList) {
      @Suppress("UNCHECKED_CAST")
      val resultMap = item as Map<String, Any?>
      val path = resultMap["path"] as? String ?: ""
      builder.addResults(
        Plugin.MockServerResult.newBuilder()
          .setPath(path)
          .setError(resultMap["error"] as? String ?: "")
          .addAllMismatches(luaValueToContentMismatches(path, resultMap["mismatches"]))
          .build()
      )
    }
    return builder.build()
  }

  /**
   * Converts the map returned by the Lua `prepare_interaction_for_verification` function,
   * shaped as either `{ error = "..." }` or `{ interaction_data = { body, metadata } }`, into a
   * `VerificationPreparationResponse`.
   */
  private fun luaToVerificationPreparationResponse(value: Any?): Plugin.VerificationPreparationResponse {
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?> ?: emptyMap()
    val error = map["error"] as? String
    if (error != null) {
      return Plugin.VerificationPreparationResponse.newBuilder().setError(error).build()
    }
    val data = luaToInteractionData(map["interaction_data"])
      ?: throw IllegalStateException(
        "Lua prepare_interaction_for_verification() must return either 'error' or 'interaction_data'"
      )
    return Plugin.VerificationPreparationResponse.newBuilder().setInteractionData(data).build()
  }

  /**
   * Converts a single Lua verification mismatch (a plain error string, or a mismatch map shaped
   * like a `match_contents` mismatch) into a `VerificationResultItem`.
   */
  private fun luaToVerificationResultItem(value: Any?): Plugin.VerificationResultItem {
    return when (value) {
      is String -> Plugin.VerificationResultItem.newBuilder().setError(value).build()
      is Map<*, *> -> {
        @Suppress("UNCHECKED_CAST")
        val map = value as Map<String, Any?>
        val mismatchBuilder = Plugin.ContentMismatch.newBuilder()
          .setMismatch(map["mismatch"] as? String ?: "")
          .setPath(map["path"] as? String ?: "")
        map["expected"]?.let { mismatchBuilder.expected = BytesValue.of(ByteString.copyFromUtf8(it.toString())) }
        map["actual"]?.let { mismatchBuilder.actual = BytesValue.of(ByteString.copyFromUtf8(it.toString())) }
        (map["diff"] as? String)?.let { mismatchBuilder.diff = it }
        (map["mismatch_type"] as? String)?.let { mismatchBuilder.mismatchType = it }
        Plugin.VerificationResultItem.newBuilder().setMismatch(mismatchBuilder.build()).build()
      }
      else -> throw IllegalStateException("Expected a mismatch string or table from Lua, got $value")
    }
  }

  /**
   * Converts the map returned by the Lua `verify_interaction` function, shaped as either
   * `{ error = "..." }` or
   * `{ result = { success, response_data, mismatches = { ... }, output = { ... } } }`, into a
   * `VerifyInteractionResponse`.
   */
  private fun luaToVerifyInteractionResponse(value: Any?): Plugin.VerifyInteractionResponse {
    @Suppress("UNCHECKED_CAST")
    val map = value as? Map<String, Any?> ?: emptyMap()
    val error = map["error"] as? String
    if (error != null) {
      return Plugin.VerifyInteractionResponse.newBuilder().setError(error).build()
    }
    @Suppress("UNCHECKED_CAST")
    val resultMap = map["result"] as? Map<String, Any?>
      ?: throw IllegalStateException("Lua verify_interaction() must return either 'error' or 'result'")

    val builder = Plugin.VerificationResult.newBuilder()
      .setSuccess(resultMap["success"] as? Boolean ?: false)
    luaToInteractionData(resultMap["response_data"])?.let { builder.responseData = it }
    @Suppress("UNCHECKED_CAST")
    val mismatches = resultMap["mismatches"] as? List<Any?> ?: emptyList()
    for (mismatch in mismatches) {
      builder.addMismatches(luaToVerificationResultItem(mismatch))
    }
    @Suppress("UNCHECKED_CAST")
    val output = resultMap["output"] as? List<Any?> ?: emptyList()
    builder.addAllOutput(output.map { it.toString() })

    return Plugin.VerifyInteractionResponse.newBuilder().setResult(builder.build()).build()
  }
}
