package io.pact.protobuf.plugin

import au.com.dius.pact.core.matchers.MatchingContext
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.matchingrules.MatchingRule
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import au.com.dius.pact.core.model.matchingrules.expressions.MatchingRuleDefinition.parseMatchingRuleDefinition
import au.com.dius.pact.core.support.Json.toJson
import au.com.dius.pact.core.support.isNotEmpty
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.google.common.io.BaseEncoding
import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.DescriptorProtos
import com.google.protobuf.Descriptors
import com.google.protobuf.DynamicMessage
import com.google.protobuf.Empty
import com.google.protobuf.ProtocolStringList
import com.google.protobuf.Value
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.Status
import io.grpc.StatusRuntimeException
import io.pact.plugin.PactPluginGrpcKt
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.Utils.structToJson
import io.pact.plugins.jvm.core.Utils.toProtoStruct
import kotlinx.coroutines.*
import mu.KLogging
import java.lang.Double.parseDouble
import java.lang.Float.parseFloat
import java.lang.Integer.parseInt
import java.lang.Long.parseLong
import java.nio.file.Path
import java.security.MessageDigest
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
        .setType(Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER)
        .setKey("protobuf")
        .putValues("content-types", "application/protobuf")
      this.addCatalogueBuilder()
        .setType(Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR)
        .setKey("protobuf")
        .putValues("content-types", "application/protobuf")
    }.build()
  }

  override suspend fun updateCatalogue(request: Plugin.Catalogue): Empty {
    logger.debug { "Got update catalogue request: TODO" }
    return Empty.newBuilder().build()
  }

  override suspend fun compareContents(request: Plugin.CompareContentsRequest): Plugin.CompareContentsResponse {
    try {
      val interactionConfig = request.pluginConfiguration.interactionConfiguration.fieldsMap
      val messageKey = interactionConfig["descriptorKey"]?.stringValue
      if (messageKey == null) {
        logger.error { "Plugin configuration item with key 'descriptorKey' is required" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Plugin configuration item with key 'descriptorKey' is required")
          .build()
      }

      logger.debug { "Pact level configuration keys: ${request.pluginConfiguration.pactConfiguration.fieldsMap.keys}" }
      val descriptorBytesEncoded =
        request.pluginConfiguration.pactConfiguration.fieldsMap[messageKey]?.structValue?.fieldsMap?.get("protoDescriptors")?.stringValue
      if (descriptorBytesEncoded == null) {
        logger.error { "Plugin configuration item with key '$messageKey' is required" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Plugin configuration item with key '$messageKey' is required")
          .build()
      }

      val message = interactionConfig["message"]?.stringValue
      if (message == null) {
        logger.error { "Plugin configuration item with key 'message' is required" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Plugin configuration item with key 'message' is required")
          .build()
      }

      logger.debug { "Received compareContents request for message $message" }

      val descriptorBytes = Base64.getDecoder().decode(descriptorBytesEncoded)
      logger.debug { "Protobuf file descriptor set is ${descriptorBytes.size} bytes" }
      val digest = MessageDigest.getInstance("MD5")
      digest.update(descriptorBytes)
      val descriptorHash = BaseEncoding.base16().lowerCase().encode(digest.digest());
      if (descriptorHash != messageKey) {
        logger.error { "Protobuf descriptors checksum failed. Expected $messageKey but got $descriptorHash" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Protobuf descriptors checksum failed. Expected $messageKey but got $descriptorHash")
          .build()
      }

      val descriptors = withContext(Dispatchers.IO) {
        DescriptorProtos.FileDescriptorSet.parseFrom(descriptorBytes)
      }
      val fileDescriptors = descriptors.fileList.associateBy { it.name }
      logger.debug { "Looking for message '$message'" }
      val fileDescriptorForMessage = fileDescriptors.entries.find { entry ->
        logger.debug { "Looking for message in file descriptor ${entry.key}" }
        entry.value.messageTypeList.any {
          it.name == message
        }
      }?.value
      if (fileDescriptorForMessage == null) {
        logger.error { "Did not find the Protobuf file descriptor containing message '$message'" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Did not find the Protobuf file descriptor containing message '$message'")
          .build()
      }

      val fileDesc = Descriptors.FileDescriptor.buildFrom(
        fileDescriptorForMessage,
        buildDependencies(fileDescriptors, fileDescriptorForMessage.dependencyList)
      )
      val descriptor = fileDesc.findMessageTypeByName(message)
      if (descriptor == null) {
        logger.error { "Did not find the Protobuf descriptor for message '$message'" }
        return Plugin.CompareContentsResponse.newBuilder()
          .setError("Did not find the Protobuf descriptor for message '$message'")
          .build()
      }

      val expectedMessage = DynamicMessage.parseFrom(descriptor, request.expected.content.value)
      logger.debug { "expectedMessage = \n$expectedMessage" }
      val actualMessage = DynamicMessage.parseFrom(descriptor, request.actual.content.value)
      logger.debug { "actualMessage = \n$actualMessage" }
      val matchingContext =
        MatchingContext(MatchingRuleCategory("body", request.rulesMap.entries.associate { (key, rules) ->
          key to MatchingRuleGroup(rules.ruleList.map {
            MatchingRule.create(it.type, structToJson(it.values))
          }.toMutableList())
        }.toMutableMap()), request.allowUnexpectedKeys)
      val result = ProtobufContentMatcher.compare(expectedMessage, actualMessage, matchingContext)
      logger.debug { "result = $result" }

      val response = Plugin.CompareContentsResponse.newBuilder()
      for (item in result.bodyResults) {
        response.putResults(item.key, Plugin.ContentMismatches.newBuilder().addAllMismatches(item.result.map {
          val builder = Plugin.ContentMismatch.newBuilder()
            .setExpected(
              BytesValue.newBuilder().setValue(ByteString.copyFrom(it.expected.toString().toByteArray())).build()
            )
            .setActual(
              BytesValue.newBuilder().setValue(ByteString.copyFrom(it.actual.toString().toByteArray())).build()
            )
            .setMismatch(it.mismatch)
            .setPath(it.path)
          if (it.diff.isNotEmpty()) {
            builder.diff = it.diff
          }
          builder.build()
        }).build())
      }

      return response.build()
    } catch (ex: Exception) {
      logger.error(ex) { "Failed to generate response" }
      return Plugin.CompareContentsResponse.newBuilder()
        .setError(ex.message)
        .build()
    }
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
        logger.error { "Config item with key 'message-type' and the protobuf message name is required" }
        throw StatusRuntimeException(Status.INVALID_ARGUMENT)
      }

      logger.debug { "Parsing proto file" }
      val protoFile = Path.of(config["proto"]!!.stringValue)
      val protoResult = ProtoParser.parseProtoFile(protoFile)
      val descriptorBytes = protoResult.toByteArray()
      logger.debug { "Protobuf file descriptor set is ${descriptorBytes.size} bytes" }
      val digest = MessageDigest.getInstance("MD5")
      digest.update(descriptorBytes)
      val descriptorHash = BaseEncoding.base16().lowerCase().encode(digest.digest());

      logger.debug { "Parsed proto file OK, file descriptors = ${protoResult.fileList.map { it.name }}" }

      val fileDescriptors = protoResult.fileList.associateBy { it.name }
      val fileProtoDesc = fileDescriptors[protoFile.fileName.toString()]
      if (fileProtoDesc == null) {
        logger.error { "Did not find a file proto descriptor for $protoFile" }
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
      val fileDesc = Descriptors.FileDescriptor.buildFrom(
        fileProtoDesc,
        buildDependencies(fileDescriptors, fileProtoDesc.dependencyList)
      )
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
            when (field.type) {
              Descriptors.FieldDescriptor.Type.MESSAGE -> {
                val messageValue = configureMessageField(listOf("$"), field, value, matchingRules, generators)
                logger.debug { "Setting field $field to value '$messageValue'" }
                if (messageValue != null) {
                  messageBuilder.setField(field, messageValue)
                }
              }
              else -> {
                val fieldValue = buildFieldValue(listOf("$"), field, value, matchingRules, generators)
                logger.debug { "Setting field $field to value '$fieldValue'" }
                if (fieldValue != null) {
                  messageBuilder.setField(field, fieldValue)
                }
              }
            }
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
        .setContentTypeHint(Plugin.Body.ContentTypeHint.BINARY)

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
        builder.putGenerators(
          key, Plugin.Generator.newBuilder()
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
        .putAllFields(
          mapOf(
            "protoFile" to Value.newBuilder().setStringValue(fileContents).build(),
            "protoDescriptors" to Value.newBuilder().setStringValue(Base64.getEncoder().encodeToString(descriptorBytes))
              .build()
          )
        )
        .build()
      pluginConfigurationBuilder.pactConfigurationBuilder.putAllFields(
        mapOf(descriptorHash.toString() to valueBuilder.build())
      )
      pluginConfigurationBuilder.interactionConfigurationBuilder
        .putFields("message", Value.newBuilder().setStringValue(message).build())
        .putFields("descriptorKey", Value.newBuilder().setStringValue(descriptorHash.toString()).build())

      builder.build()
    } catch (ex: StatusRuntimeException) {
      throw ex
    } catch (ex: RuntimeException) {
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

    private fun buildFieldValue(
      path: List<String>,
      field: Descriptors.FieldDescriptor,
      value: Value?,
      matchingRules: MatchingRuleCategory,
      generators: MutableMap<String, Generator>
    ): Any? {
      return if (value != null) {
        when (val ruleDefinition = parseMatchingRuleDefinition(value.stringValue)) {
          is Ok -> {
            val (fieldValue, rule, generator) = ruleDefinition.value
            val fieldPath = path.joinToString(".") + "." + field.name
            if (rule != null) {
              matchingRules.addRule(fieldPath, rule)
            }
            if (generator != null) {
              generators[fieldPath] = generator
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
      logger.debug { "Creating value for type ${field.type.javaType} from '$fieldValue'" }
      return when (field.type.javaType) {
        Descriptors.FieldDescriptor.JavaType.INT -> parseInt(fieldValue)
        Descriptors.FieldDescriptor.JavaType.LONG -> parseLong(fieldValue)
        Descriptors.FieldDescriptor.JavaType.FLOAT -> parseFloat(fieldValue)
        Descriptors.FieldDescriptor.JavaType.DOUBLE -> parseDouble(fieldValue)
        Descriptors.FieldDescriptor.JavaType.BOOLEAN -> fieldValue == "true"
        Descriptors.FieldDescriptor.JavaType.STRING -> fieldValue
        Descriptors.FieldDescriptor.JavaType.BYTE_STRING -> ByteString.copyFromUtf8(fieldValue)
        Descriptors.FieldDescriptor.JavaType.ENUM -> field.enumType.findValueByName(fieldValue)
        Descriptors.FieldDescriptor.JavaType.MESSAGE -> {
          if (field.messageType.fullName == "google.protobuf.BytesValue") {
            BytesValue.newBuilder().setValue(ByteString.copyFromUtf8(fieldValue)).build()
          } else {
            logger.error { "field ${field.name} is a Message type" }
            throw StatusRuntimeException(Status.INVALID_ARGUMENT)
          }
        }
        null -> null
      }
    }

    private fun configureMessageField(
      path: List<String>,
      messageField: Descriptors.FieldDescriptor,
      value: Value?,
      matchingRules: MatchingRuleCategory,
      generators: MutableMap<String, Generator>
    ): Any? {
      logger.debug { "Configuring message field $messageField (type ${messageField.messageType.fullName})" }
      return if (value != null) {
        if (messageField.messageType.fullName == "google.protobuf.BytesValue") {
          logger.debug { "Field is a Protobuf BytesValue" }
          when (value.kindCase) {
            Value.KindCase.STRING_VALUE -> buildFieldValue(path, messageField, value, matchingRules, generators)
            else -> {
              logger.error {
                "Fields of type google.protobuf.BytesValue must be configured with a single string value"
              }
              throw StatusRuntimeException(Status.INVALID_ARGUMENT)
            }
          }
        } else {
          logger.debug { "Configuring the message from config map" }
          when (value.kindCase) {
            Value.KindCase.STRUCT_VALUE -> createDynamicMessage(messageField, value, path, matchingRules, generators)
            else -> {
              logger.error { "For message fields, you need to define a Map of expected fields. Got $value" }
              throw StatusRuntimeException(Status.INVALID_ARGUMENT)
            }
          }
        }
      } else {
        null
      }
    }

    private fun createDynamicMessage(
      messageField: Descriptors.FieldDescriptor,
      value: Value,
      path: List<String>,
      matchingRules: MatchingRuleCategory,
      generators: MutableMap<String, Generator>
    ): DynamicMessage? {
      val messageDescriptor = messageField.messageType
      val messageBuilder = DynamicMessage.newBuilder(messageDescriptor)

      for ((key, v) in value.structValue.fieldsMap) {
        val field = messageDescriptor.findFieldByName(key)
        if (field != null) {
          val fieldPath = path + messageField.name
          when (field.type) {
            Descriptors.FieldDescriptor.Type.MESSAGE -> {
              val messageValue = configureMessageField(fieldPath, field, v, matchingRules, generators)
              logger.debug { "Setting field $field to value $messageValue" }
              if (messageValue != null) {
                messageBuilder.setField(field, messageValue)
              }
            }
            else -> {
              val fieldValue = buildFieldValue(fieldPath, field, v, matchingRules, generators)
              logger.debug { "Setting field $field to value $fieldValue" }
              if (fieldValue != null) {
                messageBuilder.setField(field, fieldValue)
              }
            }
          }
        } else {
          logger.error { "Message $messageField has no field $key" }
          throw StatusRuntimeException(Status.INVALID_ARGUMENT)
        }
      }

      return messageBuilder.build()
    }
  }
}

fun main() {
  val server = PluginApp()
  server.start()
  server.blockUntilShutdown()
}
