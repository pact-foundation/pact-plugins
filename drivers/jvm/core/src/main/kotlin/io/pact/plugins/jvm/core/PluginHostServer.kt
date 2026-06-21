package io.pact.plugins.jvm.core

import com.google.protobuf.Empty
import io.github.oshai.kotlinlogging.KotlinLogging
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.stub.StreamObserver
import io.pact.plugin.v2.PluginHostGrpc
import io.pact.plugin.v2.PluginV2
import java.util.concurrent.ConcurrentHashMap
import org.slf4j.MDC

private val logger = KotlinLogging.logger {}

internal class PluginHostGrpcService : PluginHostGrpc.PluginHostImplBase() {
  override fun log(request: PluginV2.LogMessage, responseObserver: StreamObserver<Empty>) {
    val instanceId = request.pluginInstanceId
    val pluginName = PluginHostServer.pluginNameForInstance(instanceId) ?: instanceId
    val target = request.target.ifEmpty { "io.pact.plugin.$pluginName" }
    val pluginLogger = KotlinLogging.logger(target)
    val message = "[instance:$instanceId] ${request.message}"
    val testRunId = request.testRunId.ifEmpty { null }
    if (testRunId != null) MDC.put("testRunId", testRunId)
    try {
      when (request.level.uppercase()) {
        "TRACE" -> pluginLogger.trace { message }
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

  @Synchronized
  fun ensureRunning(): Int {
    if (server == null) {
      server = ServerBuilder.forPort(0)
        .addService(PluginHostGrpcService())
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
