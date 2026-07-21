package io.pact.plugins.jvm.core

import com.google.protobuf.Empty
import io.github.oshai.kotlinlogging.KotlinLogging
import io.grpc.Context
import io.grpc.Contexts
import io.grpc.Metadata
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.ServerCall
import io.grpc.ServerCallHandler
import io.grpc.ServerInterceptor
import io.grpc.ServerInterceptors
import io.grpc.Status
import io.grpc.stub.StreamObserver
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginHostGrpc
import io.pact.plugin.v2.PluginV2
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import org.slf4j.MDC

private val logger = KotlinLogging.logger {}

private val TRANSPORT_TARGET_PREFIXES = listOf("h2::", "tower::", "tonic::", "hyper_util::", "hyper::")

private fun isTransportTarget(target: String) =
  TRANSPORT_TARGET_PREFIXES.any { target.startsWith(it) }

private val CALL_CHAIN_ID_HEADER: Metadata.Key<String> =
  Metadata.Key.of(CallChain.CALL_CHAIN_ID_METADATA_KEY, Metadata.ASCII_STRING_MARSHALLER)
private val DEADLINE_HEADER: Metadata.Key<String> =
  Metadata.Key.of(CallChain.DEADLINE_METADATA_KEY, Metadata.ASCII_STRING_MARSHALLER)
private val CALL_CHAIN_ID_CONTEXT_KEY: Context.Key<String> = Context.key(CallChain.CALL_CHAIN_ID_METADATA_KEY)
private val DEADLINE_CONTEXT_KEY: Context.Key<Long> = Context.key(CallChain.DEADLINE_METADATA_KEY)

/**
 * Reads the call-chain ID/deadline gRPC metadata off an incoming callback request and stashes
 * them in the request's [Context] so [PluginHostGrpcService]'s handlers can read them back. See
 * [CallChain].
 */
internal class CallChainServerInterceptor : ServerInterceptor {
  override fun <ReqT, RespT> interceptCall(
    call: ServerCall<ReqT, RespT>,
    headers: Metadata,
    next: ServerCallHandler<ReqT, RespT>
  ): ServerCall.Listener<ReqT> {
    val context = Context.current()
      .withValue(CALL_CHAIN_ID_CONTEXT_KEY, headers.get(CALL_CHAIN_ID_HEADER))
      .withValue(DEADLINE_CONTEXT_KEY, headers.get(DEADLINE_HEADER)?.toLongOrNull())
    return Contexts.interceptCall(context, call, headers, next)
  }
}

/**
 * Extract the call-chain ID and deadline stashed by [CallChainServerInterceptor], falling back to
 * a fresh chain and the default budget if either is missing - defensive handling for a plugin
 * that didn't propagate the metadata it was given. See [CallChain].
 */
private fun callChainContext(): Pair<String, Long> {
  val chainId = CALL_CHAIN_ID_CONTEXT_KEY.get() ?: run {
    logger.warn { "Callback request had no '${CallChain.CALL_CHAIN_ID_METADATA_KEY}' metadata, starting a new call chain" }
    CallChain.newCallChainId()
  }
  val deadlineMs = DEADLINE_CONTEXT_KEY.get() ?: run {
    logger.warn { "Callback request had no '${CallChain.DEADLINE_METADATA_KEY}' metadata, using the default budget" }
    CallChain.defaultDeadlineMs()
  }
  return chainId to deadlineMs
}

internal class PluginHostGrpcService : PluginHostGrpc.PluginHostImplBase() {
  override fun log(request: PluginV2.LogMessage, responseObserver: StreamObserver<Empty>) {
    if (request.level.uppercase() == "TRACE" || isTransportTarget(request.target)) {
      responseObserver.onNext(Empty.getDefaultInstance())
      responseObserver.onCompleted()
      return
    }
    val instanceId = request.pluginInstanceId
    val pluginName = PluginHostServer.pluginNameForInstance(instanceId)
    val loggerName = if (!pluginName.isNullOrEmpty()) "io.pact.plugin.$pluginName" else "io.pact.plugin"
    val pluginLogger = KotlinLogging.logger(loggerName)
    val shortId = instanceId.take(8)
    val prefix = if (!pluginName.isNullOrEmpty()) {
      if (shortId.isNotEmpty()) "[$pluginName:$shortId]" else "[$pluginName]"
    } else {
      if (shortId.isNotEmpty()) "[plugin:$shortId]" else "[plugin]"
    }
    val testRunId = request.testRunId.ifEmpty { null }
    val message = if (testRunId != null) "$prefix [$testRunId] ${request.message}" else "$prefix ${request.message}"
    if (testRunId != null) MDC.put("testRunId", testRunId)
    try {
      when (request.level.uppercase()) {
        "DEBUG" -> pluginLogger.debug { message }
        "INFO"  -> pluginLogger.info { message }
        "WARN"  -> pluginLogger.warn { message }
        "ERROR" -> pluginLogger.error { message }
        else    -> pluginLogger.debug { message }
      }
    } finally {
      if (testRunId != null) MDC.remove("testRunId")
    }
    responseObserver.onNext(Empty.getDefaultInstance())
    responseObserver.onCompleted()
  }

  override fun compareContents(
    request: PluginV2.HostCompareContentsRequest,
    responseObserver: StreamObserver<PluginV2.CompareContentsResponse>
  ) {
    val (chainId, deadlineMs) = callChainContext()
    try {
      if (CallChain.isExpired(deadlineMs)) {
        throw PactCallChainDeadlineExceededException(chainId)
      }
      CallChain.pushCall(chainId, request.entryKey).use {
        val v1Request = convertMessage(request.request, Plugin.CompareContentsRequest.parser())
        val response = when (val resolved = CatalogueManager.resolveCapability(request.entryKey, CatalogueEntryType.CONTENT_MATCHER)) {
          is ResolvedCapability.Core -> {
            val handler = CoreCapabilityRegistry.contentMatcher(resolved.key)
              ?: throw PactCoreCapabilityNotFoundException(resolved.key)
            handler.compareContents(v1Request)
          }
          is ResolvedCapability.Plugin -> {
            val plugin = DefaultPluginManager.lookupPlugin(resolved.pluginName, null)
              ?: throw PactPluginNotFoundException(resolved.pluginName, null)
            plugin.withRpcClient { client -> client.compareContentsWithChain(v1Request, chainId, deadlineMs) }
          }
        }
        responseObserver.onNext(convertMessage(response, PluginV2.CompareContentsResponse.parser()))
        responseObserver.onCompleted()
      }
    } catch (ex: Exception) {
      responseObserver.onError(statusFor(ex).asRuntimeException())
    }
  }

  override fun generateContent(
    request: PluginV2.HostGenerateContentRequest,
    responseObserver: StreamObserver<PluginV2.GenerateContentResponse>
  ) {
    val (chainId, deadlineMs) = callChainContext()
    try {
      if (CallChain.isExpired(deadlineMs)) {
        throw PactCallChainDeadlineExceededException(chainId)
      }
      CallChain.pushCall(chainId, request.entryKey).use {
        val v1Request = convertMessage(request.request, Plugin.GenerateContentRequest.parser())
        val response = when (val resolved = CatalogueManager.resolveCapability(request.entryKey, CatalogueEntryType.CONTENT_GENERATOR)) {
          is ResolvedCapability.Core -> {
            val handler = CoreCapabilityRegistry.contentGenerator(resolved.key)
              ?: throw PactCoreCapabilityNotFoundException(resolved.key)
            handler.generateContent(v1Request)
          }
          is ResolvedCapability.Plugin -> {
            val plugin = DefaultPluginManager.lookupPlugin(resolved.pluginName, null)
              ?: throw PactPluginNotFoundException(resolved.pluginName, null)
            plugin.withRpcClient { client -> client.generateContentWithChain(v1Request, chainId, deadlineMs) }
          }
        }
        responseObserver.onNext(convertMessage(response, PluginV2.GenerateContentResponse.parser()))
        responseObserver.onCompleted()
      }
    } catch (ex: Exception) {
      responseObserver.onError(statusFor(ex).asRuntimeException())
    }
  }
}

/** Map a callback dispatch failure to the appropriate gRPC status code. */
private fun statusFor(ex: Exception): Status = when (ex) {
  is PactCallChainCycleException -> Status.ALREADY_EXISTS.withDescription(ex.message).withCause(ex)
  is PactCallChainDeadlineExceededException -> Status.DEADLINE_EXCEEDED.withDescription(ex.message).withCause(ex)
  is PactCatalogueEntryNotFoundException -> Status.NOT_FOUND.withDescription(ex.message).withCause(ex)
  is PactCatalogueEntryTypeMismatchException -> Status.NOT_FOUND.withDescription(ex.message).withCause(ex)
  is PactCoreCapabilityNotFoundException -> Status.NOT_FOUND.withDescription(ex.message).withCause(ex)
  is PactPluginNotFoundException -> Status.NOT_FOUND.withDescription(ex.message).withCause(ex)
  else -> {
    logger.error(ex) { "PluginHost callback dispatch failed" }
    Status.INTERNAL.withDescription(ex.message).withCause(ex)
  }
}

/**
 * Singleton gRPC server that plugins connect to for forwarding structured log records.
 * Started at most once per JVM; the bound port is passed to each plugin via the
 * PACT_PLUGIN_HOST environment variable.
 */
object PluginHostServer {
  private var server: Server? = null
  private var port: Int = 0
  private val instanceNames = ConcurrentHashMap<String, String>()
  private val threadIndex = AtomicInteger(0)

  @Synchronized
  fun ensureRunning(): Int {
    if (server == null) {
      val executor = Executors.newCachedThreadPool { r ->
        Thread(r).apply {
          name = "pact-plugin-log-${threadIndex.incrementAndGet()}"
          isDaemon = true
        }
      }
      server = ServerBuilder.forPort(0)
        .addService(ServerInterceptors.intercept(PluginHostGrpcService(), CallChainServerInterceptor()))
        .executor(executor)
        .build()
        .start()
      port = server!!.port
      logger.info { "PluginHost gRPC server started on port $port" }
    }
    return port
  }

  fun registerInstance(instanceId: String, pluginName: String) {
    instanceNames[instanceId] = pluginName
  }

  fun deregisterInstance(instanceId: String) {
    instanceNames.remove(instanceId)
  }

  fun pluginNameForInstance(instanceId: String): String? = instanceNames[instanceId]
}
