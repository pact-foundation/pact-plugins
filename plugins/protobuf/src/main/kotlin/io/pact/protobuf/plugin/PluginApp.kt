package io.pact.protobuf.plugin

import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.expressions.MatchingRuleDefinition.parseMatchingRuleDefinition
import au.com.dius.pact.core.support.Json.toJson
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.DescriptorProtos
import com.google.protobuf.Descriptors
import com.google.protobuf.DynamicMessage
import com.google.protobuf.ProtocolStringList
import com.google.protobuf.Value
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.Status
import io.grpc.StatusRuntimeException
import io.pact.plugin.PactPluginGrpcKt
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.Utils.toProtoStruct
import mu.KLogging
import org.apache.commons.lang3.builder.HashCodeBuilder
import java.io.File
import java.lang.Double.parseDouble
import java.lang.Float.parseFloat
import java.lang.Integer.parseInt
import java.lang.Long.parseLong
import java.nio.file.Path
import java.util.Base64
import java.util.UUID.randomUUID

class PluginApp(
  private val server: Server = ServerBuilder.forPort(0).addService(PactPluginService()).build(),
  private val serverKey: String = randomUUID().toString()
) {
  fun start() {
    server.start()
    println("{\"port\":${server.port}, \"serverKey\":\"$serverKey\"}")
    Runtime.getRuntime().addShutdownHook(
      Thread {
        println("*** shutting down gRPC server since JVM is shutting down")
        server.shutdownNow()
        println("*** server shut down")
      }
    )
  }

  fun stop() {
    server.shutdown()
  }

  fun blockUntilShutdown() {
    server.awaitTermination()
  }
}

class PactPluginService : PactPluginGrpcKt.PactPluginCoroutineImplBase() {
  override suspend fun initPlugin(request: Plugin.InitPluginRequest): Plugin.InitPluginResponse {
    logger.debug { "Init request from ${request.implementation}/${request.version}" }
    return Plugin.InitPluginResponse.newBuilder().apply {
      this.addCatalogueBuilder()
        .setType("content-matcher")
        .setKey("protobuf")
        .putValues("content-types", "application/protobuf")
      this.addCatalogueBuilder()
        .setType("content-generator")
        .setKey("protobuf")
        .putValues("content-types", "application/protobuf")
    }.build()
  }

  override suspend fun updateCatalogue(request: Plugin.Catalogue): Plugin.Void {
    logger.debug { "Got update catalogue request: TODO" }
    return Plugin.Void.newBuilder().build()
  }

  override suspend fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse {
    return super.compareContents(request)
  }

  override suspend fun configureInteraction(request: Plugin.ConfigureInteractionRequest): Plugin.ConfigureInteractionResponse {
    logger.debug { "Received configureInteraction request for '${request.contentType}'" }

    return try {
      val config = request.contentsConfig.fieldsMap
      if (!config.containsKey("proto")) {
        logger.error { "Config item with key 'proto' and path to the proto file is required" }
        throw StatusRuntimeException(Status.INVALID_ARGUMENT)
      }
      if (!config.containsKey("message-type")) {
        logger.error {"Config item with key 'message-type' and the protobuf message name is required" }
        throw StatusRuntimeException(Status.INVALID_ARGUMENT)
      }

      logger.debug { "Parsing proto file" }
      val protoFile = Path.of(config["proto"]!!.stringValue)
      val protoResult = ProtoParser.parseProtoFile(protoFile)
      val descriptorBytes = protoResult.toByteArray()
      val descriptorHash = HashCodeBuilder(897, 433)
        .append(descriptorBytes).toHashCode()

      logger.debug { "Parsed proto file OK, file descriptors = ${protoResult.fileList.map { it.name }}" }

      val fileDescriptors = protoResult.fileList.associateBy { it.name }
      val fileProtoDesc = fileDescriptors[protoFile.fileName.toString()]
      if (fileProtoDesc == null) {
        logger.error {"Did not find a file proto descriptor for $protoFile" }
        throw StatusRuntimeException(Status.ABORTED)
      }

      if (logger.isTraceEnabled) {
        logger.trace { "All message types in proto descriptor" }
        for (messageType in fileProtoDesc.messageTypeList) {
          logger.trace { messageType.toString() }
        }
      }

      val message = config["message-type"]!!.stringValue
      logger.debug { "Looking for message of type '$message'" }
      val fileDesc = Descriptors.FileDescriptor.buildFrom(fileProtoDesc,
        buildDependencies(fileDescriptors, fileProtoDesc.dependencyList))
      logger.debug { "fileDesc = $fileDesc" }
      val descriptor = fileDesc.findMessageTypeByName(message)
      if (descriptor == null) {
        logger.error { "Message '$message' was not found in proto file '$protoFile'" }
        throw StatusRuntimeException(Status.INVALID_ARGUMENT)
      }

      val messageBuilder = DynamicMessage.newBuilder(descriptor)

      val matchingRules = MatchingRuleCategory("body")
      val generators = mutableMapOf<String, Generator>()

      logger.debug { "Building message from Protobuf descriptor" }
      for ((key, value) in config) {
        if (!metaKeys.contains(key)) {
          val field = descriptor.findFieldByName(key)
          if (field != null) {
            messageBuilder.setField(field, buildFieldValue(field, value, matchingRules, generators))
          } else {
            logger.error { "Message $message has no field $key" }
            throw StatusRuntimeException(Status.INVALID_ARGUMENT)
          }
        }
      }

      logger.debug { "Returning response" }
      val builder = Plugin.ConfigureInteractionResponse.newBuilder()

      builder.contentsBuilder
        .setContentType("application/protobuf;message=$message")
        .setContent(BytesValue.newBuilder().setValue(messageBuilder.build().toByteString()).build())
        .setContentTypeOverride(Plugin.Body.ContentTypeOverride.BINARY)

      for ((key, rules) in matchingRules.matchingRules) {
        val rulesBuilder = Plugin.MatchingRules.newBuilder()

        for (rule in rules.rules) {
          rulesBuilder.addRule(
            Plugin.MatchingRule.newBuilder()
            .setType(rule.name)
            .setValues(toProtoStruct(rule.attributes))
              .build()
          )
        }

        builder.putRules(key, rulesBuilder.build())
      }

      for ((key, generator) in generators) {
        builder.putGenerators(key, Plugin.Generator.newBuilder()
          .setType(generator.type)
          .setValues(toProtoStruct(toJson(generator.toMap(PactSpecVersion.V4)).asObject()!!.entries))
          .build()
        )
      }

      val fileContents = protoFile.toFile().readText()
      val pluginConfigurationBuilder = builder.pluginConfigurationBuilder
      val valueBuilder = Value.newBuilder()
      val structValueBuilder = valueBuilder.structValueBuilder
      structValueBuilder
          .putAllFields(mapOf(
            "protoFile" to Value.newBuilder().setStringValue(fileContents).build(),
            "protoDescriptors" to Value.newBuilder().setStringValue(Base64.getEncoder().encodeToString(descriptorBytes)).build()
          ))
        .build()
      pluginConfigurationBuilder.pactConfigurationBuilder.putAllFields(
        mapOf(descriptorHash.toString() to valueBuilder.build())
      )
      pluginConfigurationBuilder.interactionConfigurationBuilder
        .putFields("message", Value.newBuilder().setStringValue(message).build())
        .putFields("descriptorKey", Value.newBuilder().setStringValue(descriptorHash.toString()).build())

      builder.build()
    } catch (ex: Exception) {
      logger.error(ex) { "Failed with an unknown exception" }
      throw StatusRuntimeException(Status.ABORTED)
    }
  }

  private fun buildDependencies(
    fileDescriptors: Map<String, DescriptorProtos.FileDescriptorProto>,
    dependencyList: ProtocolStringList
  ): Array<Descriptors.FileDescriptor> {
    logger.debug { "building dependencies for $dependencyList" }
    return dependencyList.map {
      val fileProtoDesc = fileDescriptors[it]!!
      Descriptors.FileDescriptor.buildFrom(fileProtoDesc,
        buildDependencies(fileDescriptors, fileProtoDesc.dependencyList))
    }.toTypedArray()
  }

  override suspend fun generateContent(request: Plugin.GenerateContentRequest): Plugin.GenerateContentResponse {
    return super.generateContent(request)
  }

  companion object : KLogging() {
    val metaKeys = setOf("proto", "message-type", "content-type")

    // version=matching(semver, '0.0.0'),
    // implementation=notEmpty('pact-jvm-driver'),
    private fun buildFieldValue(
      field: Descriptors.FieldDescriptor,
      value: Value?,
      matchingRules: MatchingRuleCategory,
      generators: MutableMap<String, Generator>
    ): Any? {
      return if (value != null) {
        when (val ruleDefinition = parseMatchingRuleDefinition(value.stringValue)) {
          is Ok -> {
            val (fieldValue, rule, generator) = ruleDefinition.value
            val path = "$." + field.name
            if (rule != null) {
              matchingRules.addRule(path, rule)
            }
            if (generator != null) {
              generators[path] = generator
            }
            valueForType(fieldValue, field)
          }
          is Err -> {
            logger.error { "'${value.stringValue}' is not a valid matching rule definition " +
              "- ${ruleDefinition.error}" }
            throw StatusRuntimeException(Status.INVALID_ARGUMENT)
          }
        }
      } else {
        null
      }
    }

    private fun valueForType(fieldValue: String, field: Descriptors.FieldDescriptor): Any? {
      return when (field.type.javaType) {
        Descriptors.FieldDescriptor.JavaType.INT -> parseInt(fieldValue)
        Descriptors.FieldDescriptor.JavaType.LONG -> parseLong(fieldValue)
        Descriptors.FieldDescriptor.JavaType.FLOAT -> parseFloat(fieldValue)
        Descriptors.FieldDescriptor.JavaType.DOUBLE -> parseDouble(fieldValue)
        Descriptors.FieldDescriptor.JavaType.BOOLEAN -> fieldValue == "true"
        Descriptors.FieldDescriptor.JavaType.STRING -> fieldValue
        Descriptors.FieldDescriptor.JavaType.BYTE_STRING -> ByteString.copyFromUtf8(fieldValue)
        Descriptors.FieldDescriptor.JavaType.ENUM -> TODO("Enum support has not been implemented")
        Descriptors.FieldDescriptor.JavaType.MESSAGE -> {
          logger.error { "field ${field.name} is a Message type" }
          throw StatusRuntimeException(Status.INVALID_ARGUMENT)
        }
        null -> null
      }
    }
  }
}

fun main() {
  val server = PluginApp()
  server.start()
  server.blockUntilShutdown()
}
