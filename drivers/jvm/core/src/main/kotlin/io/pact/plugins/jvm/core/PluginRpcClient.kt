package io.pact.plugins.jvm.core

import com.google.protobuf.MessageLite
import com.google.protobuf.Parser
import com.google.protobuf.Struct
import com.google.protobuf.Value
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PactPluginGrpc as PactPluginGrpcV2
import io.pact.plugin.v2.PluginV2

enum class PluginInterfaceVersion(val value: Int) {
  V1(1),
  V2(2);

  companion object {
    fun from(value: Int): PluginInterfaceVersion? = entries.find { it.value == value }
  }
}

data class PluginInitRequest(
  val implementation: String,
  val version: String,
  val hostCapabilities: List<String> = emptyList(),
  val pluginInstanceId: String = ""
)

data class PluginInitResponse(
  val catalogueEntries: List<Plugin.CatalogueEntry>,
  val pluginCapabilities: List<String> = emptyList()
)

interface PactPluginRpcClient {
  fun initPlugin(request: PluginInitRequest): PluginInitResponse
  fun updateCatalogue(request: Plugin.Catalogue)
  fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse
  fun configureInteraction(request: Plugin.ConfigureInteractionRequest): Plugin.ConfigureInteractionResponse
  fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse
  fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse
  fun shutdownMockServer(request: Plugin.ShutdownMockServerRequest): Plugin.ShutdownMockServerResponse
  fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults
  fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse
  fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse

  fun startMockServerV2(request: PluginV2.StartMockServerRequest): Plugin.StartMockServerResponse =
    throw UnsupportedOperationException("V2 interface not supported by this plugin")
  fun prepareInteractionForVerificationV2(
    request: PluginV2.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    throw UnsupportedOperationException("V2 interface not supported by this plugin")
  fun verifyInteractionV2(
    request: PluginV2.VerifyInteractionRequest
  ): Plugin.VerifyInteractionResponse =
    throw UnsupportedOperationException("V2 interface not supported by this plugin")
}

class PactPluginV1RpcClient(
  private val stub: PactPluginGrpc.PactPluginBlockingStub
) : PactPluginRpcClient {
  override fun initPlugin(request: PluginInitRequest): PluginInitResponse {
    val response = stub.initPlugin(Plugin.InitPluginRequest.newBuilder()
      .setImplementation(request.implementation)
      .setVersion(request.version)
      .build())
    return PluginInitResponse(response.catalogueList)
  }

  override fun updateCatalogue(request: Plugin.Catalogue) {
    stub.updateCatalogue(request)
  }

  override fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse =
    stub.compareContents(request)

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse = stub.configureInteraction(request)

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse =
    stub.generateContent(request)

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse =
    stub.startMockServer(request)

  override fun shutdownMockServer(
    request: Plugin.ShutdownMockServerRequest
  ): Plugin.ShutdownMockServerResponse = stub.shutdownMockServer(request)

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults =
    stub.getMockServerResults(request)

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse = stub.prepareInteractionForVerification(request)

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse =
    stub.verifyInteraction(request)
}

class PactPluginV2RpcClient(
  private val stub: PactPluginGrpcV2.PactPluginBlockingStub
) : PactPluginRpcClient {
  override fun initPlugin(request: PluginInitRequest): PluginInitResponse {
    val response = stub.initPlugin(PluginV2.InitPluginRequest.newBuilder()
      .setImplementation(request.implementation)
      .setVersion(request.version)
      .addAllHostCapabilities(request.hostCapabilities)
      .setPluginInstanceId(request.pluginInstanceId)
      .build())

    return when (response.responseCase) {
      PluginV2.InitPluginResponse.ResponseCase.SUCCESS -> PluginInitResponse(
        response.success.catalogueList.map {
          convert(it, Plugin.CatalogueEntry.parser())
        },
        response.success.pluginCapabilitiesList
      )
      PluginV2.InitPluginResponse.ResponseCase.FAILURE -> {
        val error = buildString {
          append(response.failure.error)
          if (response.failure.missingHostCapabilitiesCount > 0) {
            append(" (missing host capabilities: ")
            append(response.failure.missingHostCapabilitiesList.joinToString(", "))
            append(')')
          }
        }
        throw IllegalStateException(error)
      }
      PluginV2.InitPluginResponse.ResponseCase.RESPONSE_NOT_SET ->
        throw IllegalStateException("Plugin returned an invalid V2 InitPlugin response")
      null -> throw IllegalStateException("Plugin returned an invalid V2 InitPlugin response")
    }
  }

  override fun updateCatalogue(request: Plugin.Catalogue) {
    stub.updateCatalogue(convert(request, PluginV2.Catalogue.parser()))
  }

  override fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse {
    var v2Request = convert(request, PluginV2.CompareContentsRequest.parser())
    val testRunId = TestContext.currentTestRunId()
    if (!v2Request.hasTestContext() && testRunId != null) {
      v2Request = v2Request.toBuilder()
        .setTestContext(
          Struct.newBuilder()
            .putFields("testRunId", Value.newBuilder().setStringValue(testRunId).build())
            .build()
        )
        .build()
    }
    return convert(stub.compareContents(v2Request), Plugin.CompareContentsResponse.parser())
  }

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse {
    var v2Request = convert(request, PluginV2.ConfigureInteractionRequest.parser())
    val testRunId = TestContext.currentTestRunId()
    if (!v2Request.hasTestContext() && testRunId != null) {
      v2Request = v2Request.toBuilder()
        .setTestContext(
          Struct.newBuilder()
            .putFields("testRunId", Value.newBuilder().setStringValue(testRunId).build())
            .build()
        )
        .build()
    }
    return convert(stub.configureInteraction(v2Request), Plugin.ConfigureInteractionResponse.parser())
  }

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse =
    convert(
      stub.generateContent(convert(request, PluginV2.GenerateContentRequest.parser())),
      Plugin.GenerateContentResponse.parser()
    )

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse =
    throw UnsupportedOperationException("V2 plugins require startMockServerV2 with structured interaction data")

  override fun shutdownMockServer(
    request: Plugin.ShutdownMockServerRequest
  ): Plugin.ShutdownMockServerResponse = convert(
    stub.shutdownMockServer(convert(request, PluginV2.MockServerRequest.parser())),
    Plugin.ShutdownMockServerResponse.parser()
  )

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults =
    convert(
      stub.getMockServerResults(convert(request, PluginV2.MockServerRequest.parser())),
      Plugin.MockServerResults.parser()
    )

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    throw UnsupportedOperationException("V2 plugins require prepareInteractionForVerificationV2 with structured interaction data")

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse =
    throw UnsupportedOperationException("V2 plugins require verifyInteractionV2 with structured interaction data")

  override fun startMockServerV2(request: PluginV2.StartMockServerRequest): Plugin.StartMockServerResponse =
    convert(stub.startMockServer(request), Plugin.StartMockServerResponse.parser())

  override fun prepareInteractionForVerificationV2(
    request: PluginV2.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    convert(stub.prepareInteractionForVerification(request), Plugin.VerificationPreparationResponse.parser())

  override fun verifyInteractionV2(
    request: PluginV2.VerifyInteractionRequest
  ): Plugin.VerifyInteractionResponse =
    convert(stub.verifyInteraction(request), Plugin.VerifyInteractionResponse.parser())

  private fun <T> convert(message: MessageLite, parser: Parser<T>): T = parser.parseFrom(message.toByteArray())
}
