package io.pact.plugins.jvm.core

import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin

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
