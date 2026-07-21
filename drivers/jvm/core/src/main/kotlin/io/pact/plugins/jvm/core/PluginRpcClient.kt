package io.pact.plugins.jvm.core

import com.google.protobuf.MessageLite
import com.google.protobuf.Parser
import com.google.protobuf.Struct
import com.google.protobuf.Value
import io.grpc.Metadata
import io.grpc.stub.AbstractStub
import io.grpc.stub.MetadataUtils
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PactPluginGrpc as PactPluginGrpcV2
import io.pact.plugin.v2.PluginV2
import java.util.concurrent.TimeUnit

/**
 * Convert a message between plugin interface versions (V1/V2 messages are structurally
 * compatible; this is a field-preserving reinterpretation via re-serialisation). Also used to
 * decode a V2 [PluginV2] message into its V1 [Plugin] equivalent (and back) when dispatching a
 * `PluginHost` callback, since core capability handlers are registered against the V1 shape - see
 * [PluginHostServer].
 */
internal fun <T> convertMessage(message: MessageLite, parser: Parser<T>): T = parser.parseFrom(message.toByteArray())

/**
 * Attach call-chain cycle detection and deadline metadata to an outbound request to a plugin, and
 * bound the stub's own deadline to the remaining budget. See [CallChain].
 */
internal fun <S : AbstractStub<S>> S.withChainContext(chainId: String, deadlineMs: Long): S {
  val metadata = Metadata()
  metadata.put(Metadata.Key.of(CallChain.CALL_CHAIN_ID_METADATA_KEY, Metadata.ASCII_STRING_MARSHALLER), chainId)
  metadata.put(Metadata.Key.of(CallChain.DEADLINE_METADATA_KEY, Metadata.ASCII_STRING_MARSHALLER), deadlineMs.toString())
  return this.withInterceptors(MetadataUtils.newAttachHeadersInterceptor(metadata))
    .withDeadlineAfter(CallChain.remaining(deadlineMs).toMillis(), TimeUnit.MILLISECONDS)
}

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

  /**
   * Send a compare contents request, propagating call-chain cycle detection and deadline metadata
   * (see [CallChain]) for transports that support it. The default implementation ignores
   * `chainId`/`deadlineMs` and delegates to [compareContents], which suits in-process runtimes
   * (Lua, WASM) where a cycle is already caught by the native call stack;
   * [PactPluginV1RpcClient]/[PactPluginV2RpcClient] override this to send the metadata over gRPC.
   */
  fun compareContentsWithChain(
    request: Plugin.CompareContentsRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.CompareContentsResponse = compareContents(request)

  /**
   * Send a generate content request, propagating call-chain cycle detection and deadline metadata
   * (see [CallChain]) for transports that support it. See [compareContentsWithChain] for the
   * default/override split.
   */
  fun generateContentWithChain(
    request: Plugin.GenerateContentRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.GenerateContentResponse = generateContent(request)

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

  override fun compareContentsWithChain(
    request: Plugin.CompareContentsRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.CompareContentsResponse = stub.withChainContext(chainId, deadlineMs).compareContents(request)

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse = stub.configureInteraction(request)

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse =
    stub.generateContent(request)

  override fun generateContentWithChain(
    request: Plugin.GenerateContentRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.GenerateContentResponse = stub.withChainContext(chainId, deadlineMs).generateContent(request)

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
          convertMessage(it, Plugin.CatalogueEntry.parser())
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
    stub.updateCatalogue(convertMessage(request, PluginV2.Catalogue.parser()))
  }

  override fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse =
    convertMessage(stub.compareContents(withTestRunId(convertMessage(request, PluginV2.CompareContentsRequest.parser()))),
      Plugin.CompareContentsResponse.parser())

  override fun compareContentsWithChain(
    request: Plugin.CompareContentsRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.CompareContentsResponse {
    val v2Request = withTestRunId(convertMessage(request, PluginV2.CompareContentsRequest.parser()))
    return convertMessage(
      stub.withChainContext(chainId, deadlineMs).compareContents(v2Request),
      Plugin.CompareContentsResponse.parser()
    )
  }

  private fun withTestRunId(request: PluginV2.CompareContentsRequest): PluginV2.CompareContentsRequest {
    val testRunId = TestContext.currentTestRunId()
    return if (testRunId != null && !request.testContext.containsFields("testRunId")) {
      val contextBuilder = if (request.hasTestContext()) request.testContext.toBuilder() else Struct.newBuilder()
      contextBuilder.putFields("testRunId", Value.newBuilder().setStringValue(testRunId).build())
      request.toBuilder().setTestContext(contextBuilder.build()).build()
    } else {
      request
    }
  }

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse {
    var v2Request = convertMessage(request, PluginV2.ConfigureInteractionRequest.parser())
    val testRunId = TestContext.currentTestRunId()
    if (testRunId != null && !v2Request.testContext.containsFields("testRunId")) {
      val contextBuilder = if (v2Request.hasTestContext()) v2Request.testContext.toBuilder()
                           else Struct.newBuilder()
      contextBuilder.putFields("testRunId", Value.newBuilder().setStringValue(testRunId).build())
      v2Request = v2Request.toBuilder().setTestContext(contextBuilder.build()).build()
    }
    return convertMessage(stub.configureInteraction(v2Request), Plugin.ConfigureInteractionResponse.parser())
  }

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse =
    convertMessage(
      stub.generateContent(convertMessage(request, PluginV2.GenerateContentRequest.parser())),
      Plugin.GenerateContentResponse.parser()
    )

  override fun generateContentWithChain(
    request: Plugin.GenerateContentRequest,
    chainId: String,
    deadlineMs: Long
  ): Plugin.GenerateContentResponse = convertMessage(
    stub.withChainContext(chainId, deadlineMs).generateContent(convertMessage(request, PluginV2.GenerateContentRequest.parser())),
    Plugin.GenerateContentResponse.parser()
  )

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse =
    throw UnsupportedOperationException("V2 plugins require startMockServerV2 with structured interaction data")

  override fun shutdownMockServer(
    request: Plugin.ShutdownMockServerRequest
  ): Plugin.ShutdownMockServerResponse = convertMessage(
    stub.shutdownMockServer(convertMessage(request, PluginV2.MockServerRequest.parser())),
    Plugin.ShutdownMockServerResponse.parser()
  )

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults =
    convertMessage(
      stub.getMockServerResults(convertMessage(request, PluginV2.MockServerRequest.parser())),
      Plugin.MockServerResults.parser()
    )

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    throw UnsupportedOperationException("V2 plugins require prepareInteractionForVerificationV2 with structured interaction data")

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse =
    throw UnsupportedOperationException("V2 plugins require verifyInteractionV2 with structured interaction data")

  override fun startMockServerV2(request: PluginV2.StartMockServerRequest): Plugin.StartMockServerResponse =
    convertMessage(stub.startMockServer(request), Plugin.StartMockServerResponse.parser())

  override fun prepareInteractionForVerificationV2(
    request: PluginV2.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse =
    convertMessage(stub.prepareInteractionForVerification(request), Plugin.VerificationPreparationResponse.parser())

  override fun verifyInteractionV2(
    request: PluginV2.VerifyInteractionRequest
  ): Plugin.VerifyInteractionResponse =
    convertMessage(stub.verifyInteraction(request), Plugin.VerifyInteractionResponse.parser())
}
