package io.pact.plugins.jvm.core

import com.google.protobuf.MessageLite
import com.google.protobuf.Parser
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PactPluginGrpc as PactPluginGrpcV2
import io.pact.plugin.v2.PluginV2

enum class PluginInterfaceVersion(val value: Int) {
  V1(1),
  V2(2);

  companion object {
    fun from(value: Int): PluginInterfaceVersion? = values().find { it.value == value }
  }
}

interface PactPluginRpcClient {
  fun initPlugin(request: Plugin.InitPluginRequest): Plugin.InitPluginResponse
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
}

class PactPluginV1RpcClient(
  private val stub: PactPluginGrpc.PactPluginBlockingStub
) : PactPluginRpcClient {
  override fun initPlugin(request: Plugin.InitPluginRequest): Plugin.InitPluginResponse = stub.initPlugin(request)

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
  override fun initPlugin(request: Plugin.InitPluginRequest): Plugin.InitPluginResponse =
    convert(
      stub.initPlugin(convert(request, PluginV2.InitPluginRequest.parser())),
      Plugin.InitPluginResponse.parser()
    )

  override fun updateCatalogue(request: Plugin.Catalogue) {
    stub.updateCatalogue(convert(request, PluginV2.Catalogue.parser()))
  }

  override fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse =
    convert(
      stub.compareContents(convert(request, PluginV2.CompareContentsRequest.parser())),
      Plugin.CompareContentsResponse.parser()
    )

  override fun configureInteraction(
    request: Plugin.ConfigureInteractionRequest
  ): Plugin.ConfigureInteractionResponse = convert(
    stub.configureInteraction(convert(request, PluginV2.ConfigureInteractionRequest.parser())),
    Plugin.ConfigureInteractionResponse.parser()
  )

  override fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse =
    convert(
      stub.generateContent(convert(request, PluginV2.GenerateContentRequest.parser())),
      Plugin.GenerateContentResponse.parser()
    )

  override fun startMockServer(request: Plugin.StartMockServerRequest): Plugin.StartMockServerResponse =
    convert(
      stub.startMockServer(convert(request, PluginV2.StartMockServerRequest.parser())),
      Plugin.StartMockServerResponse.parser()
    )

  override fun shutdownMockServer(
    request: Plugin.ShutdownMockServerRequest
  ): Plugin.ShutdownMockServerResponse = convert(
    stub.shutdownMockServer(convert(request, PluginV2.ShutdownMockServerRequest.parser())),
    Plugin.ShutdownMockServerResponse.parser()
  )

  override fun getMockServerResults(request: Plugin.MockServerRequest): Plugin.MockServerResults =
    convert(
      stub.getMockServerResults(convert(request, PluginV2.MockServerRequest.parser())),
      Plugin.MockServerResults.parser()
    )

  override fun prepareInteractionForVerification(
    request: Plugin.VerificationPreparationRequest
  ): Plugin.VerificationPreparationResponse = convert(
    stub.prepareInteractionForVerification(convert(request, PluginV2.VerificationPreparationRequest.parser())),
    Plugin.VerificationPreparationResponse.parser()
  )

  override fun verifyInteraction(request: Plugin.VerifyInteractionRequest): Plugin.VerifyInteractionResponse =
    convert(
      stub.verifyInteraction(convert(request, PluginV2.VerifyInteractionRequest.parser())),
      Plugin.VerifyInteractionResponse.parser()
    )

  private fun <T> convert(message: MessageLite, parser: Parser<T>): T = parser.parseFrom(message.toByteArray())
}
