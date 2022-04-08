package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import au.com.dius.pact.core.model.ContentTypeHint
import au.com.dius.pact.core.model.DefaultPactWriter
import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.Pact
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.V4Interaction
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.model.generators.Category
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.generators.Generators
import au.com.dius.pact.core.model.generators.createGenerator
import au.com.dius.pact.core.model.matchingrules.MatchingRule
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import au.com.dius.pact.core.model.matchingrules.RuleLogic
import au.com.dius.pact.core.support.Json.toJson
import au.com.dius.pact.core.support.Utils.lookupEnvironmentValue
import au.com.dius.pact.core.support.isNotEmpty
import au.com.dius.pact.core.support.json.JsonValue
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.mapError
import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.Struct
import com.vdurmont.semver4j.Semver
import io.grpc.CallCredentials
import io.grpc.ManagedChannel
import io.grpc.ManagedChannelBuilder
import io.grpc.Metadata
import io.grpc.stub.AbstractBlockingStub
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.PactPluginGrpc.newBlockingStub
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.Utils.fromProtoValue
import io.pact.plugins.jvm.core.Utils.handleWith
import io.pact.plugins.jvm.core.Utils.jsonToValue
import io.pact.plugins.jvm.core.Utils.mapToProtoStruct
import io.pact.plugins.jvm.core.Utils.structToJson
import io.pact.plugins.jvm.core.Utils.toProtoStruct
import io.pact.plugins.jvm.core.Utils.toProtoValue
import io.pact.plugins.jvm.core.Utils.valueToJson
import mu.KLogging
import org.apache.commons.lang3.SystemUtils
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.lang.Runtime.getRuntime
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import javax.json.Json
import javax.json.JsonObject

/**
 * Type of plugin dependency
 */
enum class PluginDependencyType {
  OSPackage, Plugin, Library, Executable
}

/**
 * Details of a plugin dependency
 */
data class PluginDependency(
  val name: String,
  val version: String?,
  val type: PluginDependencyType
)

/**
 * Manifest entry that describes a plugin
 */
interface PactPluginManifest {
  /**
   * Directory where the manifest file is
   */
  val pluginDir: File

  /**
   * Plugin interface version
   */
  val pluginInterfaceVersion: Int

  /**
   * Plugin name
   */
  val name: String

  /**
   * Plugin version
   */
  val version: String

  /**
   * Executable type. Supported types are: exec (executable binary)
   */
  val executableType: String

  /**
   * Minimum required version of the runtime/interpreter to run the plugin
   */
  val minimumRequiredVersion: String?

  /**
   * The main executable for the plugin
   */
  val entryPoint: String

  /**
   * Additional entry points for other operating systems (i.e. requiring a .bat file for Windows)
   */
  val entryPoints: Map<String, String?>

  /**
   * List of system dependencies or plugins required to be able to execute this plugin
   */
  val dependencies: List<PluginDependency>
}

data class DefaultPactPluginManifest(
  override val pluginDir: File,
  override val pluginInterfaceVersion: Int,
  override val name: String,
  override val version: String,
  override val executableType: String,
  override val minimumRequiredVersion: String?,
  override val entryPoint: String,
  override val entryPoints: Map<String, String?>,
  override val dependencies: List<PluginDependency>
): PactPluginManifest {

  fun toMap(): Map<String, Any> {
    val map = mutableMapOf<String, Any>(
      "pluginDir" to pluginDir.toString(),
      "pluginInterfaceVersion" to pluginInterfaceVersion,
      "name" to name,
      "version" to version,
      "executableType" to executableType,
      "entryPoint" to entryPoint
    )

    if (!minimumRequiredVersion.isNullOrEmpty()) {
      map["minimumRequiredVersion"] = minimumRequiredVersion
    }

    if (entryPoints.isNotEmpty()) {
      map["entryPoints"] = entryPoints
    }

    if (dependencies.isNotEmpty()) {
      map["dependencies"] = dependencies.map {
        mapOf(
          "name" to it.name,
          "version" to it.version,
          "type" to it.type.name
        )
      }
    }

    return map
  }

  companion object : KLogging() {
    @JvmStatic
    fun fromJson(pluginDir: File, pluginJson: JsonObject): PactPluginManifest {
      val entryPoints = if (pluginJson.containsKey("entryPoints")) {
        when (val ep = pluginJson["entryPoints"]) {
          is JsonObject -> ep.entries.associate { it.key to toString(it.value) }
          else -> {
            logger.warn { "entryPoints field in plugin manifest is invalid" }
            emptyMap()
          }
        }
      } else {
        emptyMap()
      }

      return DefaultPactPluginManifest(
        pluginDir,
        toInteger(pluginJson["pluginInterfaceVersion"]) ?: 1,
        toString(pluginJson["name"])!!,
        toString(pluginJson["version"])!!,
        toString(pluginJson["executableType"])!!,
        toString(pluginJson["minimumRequiredVersion"]),
        toString(pluginJson["entryPoint"])!!,
        entryPoints,
        listOf()
      )
    }
  }
}

/**
 * Interface to a running Pact Plugin
 */
interface PactPlugin {
  val manifest: PactPluginManifest
  val port: Int?
  val serverKey: String?
  val processPid: Long?
  var stub: AbstractBlockingStub<PactPluginGrpc.PactPluginBlockingStub>?
  var catalogueEntries: List<Plugin.CatalogueEntry>?
  var channel: ManagedChannel?

  /**
   * Shutdown the running plugin
   */
  fun shutdown()

  /**
   * Invoke the callback with a gRPC stub connected to the running plugin
   */
  fun <T> withGrpcStub(callback: java.util.function.Function<PactPluginGrpc.PactPluginBlockingStub, T>): T
}

/**
 * Default implementation of a Pact Plugin
 */
data class DefaultPactPlugin(
  val cp: ChildProcess,
  override val manifest: PactPluginManifest,
  override val port: Int?,
  override val serverKey: String,
  override var stub: AbstractBlockingStub<PactPluginGrpc.PactPluginBlockingStub>? = null,
  override var catalogueEntries: List<Plugin.CatalogueEntry>? = null,
  override var channel: ManagedChannel? = null
) : PactPlugin {
  override val processPid: Long
    get() = cp.pid

  override fun shutdown() {
    cp.destroy()
    if (channel != null) {
      channel!!.shutdownNow().awaitTermination(1, TimeUnit.SECONDS)
    }
  }

  override fun <T> withGrpcStub(callback: java.util.function.Function<PactPluginGrpc.PactPluginBlockingStub, T>): T {
    return callback.apply(stub as PactPluginGrpc.PactPluginBlockingStub)
  }
}

/**
 * Interface to a plugin manager that provides access to a plugin running as a child process
 */
interface PluginManager {
  /**
   * Loads the plugin by name
   */
  fun loadPlugin(name: String, version: String?): Result<PactPlugin, String>

  /**
   * Invoke the content type matcher
   */
  fun invokeContentMatcher(
    matcher: ContentMatcher,
    expected: OptionalBody,
    actual: OptionalBody,
    allowUnexpectedKeys: Boolean,
    rules: Map<String, MatchingRuleGroup>,
    pluginConfiguration: Map<String, PluginConfiguration>
  ): Plugin.CompareContentsResponse


  /**
   * Invoke the content type matcher to configure the interaction
   */
  fun configureContentMatcherInteraction(
    matcher: ContentMatcher,
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Result<List<InteractionContents>, String>

  /**
   * Invoke the content generator to generate the contents for a body
   */
  fun generateContent(
    contentGenerator: CatalogueContentGenerator,
    contentType: ContentType,
    generators: Map<String, Generator>,
    body: OptionalBody
  ): OptionalBody

  /**
   * Starts a mock server given the catalog entry for it and a Pact
   */
  fun startMockServer(catalogueEntry: CatalogueEntry, config: MockServerConfig, pact: Pact): MockServerDetails

  /**
   * Shutdowns a running mock server. Will return any errors from the mock server.
   */
  fun shutdownMockServer(mockServer: MockServerDetails): List<MockServerResults>?

  /**
   * Sets up a transport request to be made. This is the first phase when verifying, and it allows the
   * users to add additional values to any requests that are made.
   */
  fun prepareValidationForInteraction(
    transportEntry: CatalogueEntry,
    pact: V4Pact,
    interaction: V4Interaction,
    config: Map<String, Any?>
  ): Result<InteractionVerificationData, String>

  /**
   * Executes the verification of the interaction that was configured with the prepareValidationForInteraction call
   */
  fun verifyInteraction(
    transportEntry: CatalogueEntry,
    verificationData: InteractionVerificationData,
    config: Map<String, Any?>,
    pact: V4Pact,
    interaction: V4Interaction
  ): Result<InteractionVerificationResult, String>
}

object DefaultPluginManager: KLogging(), PluginManager {
  private val PLUGIN_MANIFEST_REGISTER: MutableMap<String, PactPluginManifest> = mutableMapOf()
  private val PLUGIN_REGISTER: MutableMap<String, PactPlugin> = ConcurrentHashMap()

  init {
    getRuntime().addShutdownHook(Thread {
      logger.debug { "SHUTDOWN - shutting down all plugins" }
      PLUGIN_REGISTER.forEach { (_, plugin) ->
        plugin.shutdown()
      }
    })
  }

  override fun loadPlugin(name: String, version: String?): Result<PactPlugin, String> {
    synchronized(PLUGIN_REGISTER) {
      val plugin = lookupPlugin(name, version)
      return if (plugin != null) {
        Ok(plugin)
      } else {
        when (val manifest = loadPluginManifest(name, version)) {
          is Ok -> {
            PluginMetrics.sendMetrics(manifest.value)
            initialisePlugin(manifest.value)
          }
          is Err -> Err(manifest.error)
        }
      }
    }
  }

  private fun lookupPlugin(name: String, version: String?): PactPlugin? {
    return if (version == null) {
      PLUGIN_REGISTER.filter { it.value.manifest.name == name }.entries.maxByOrNull { it.value.manifest.version }?.value
    } else {
      PLUGIN_REGISTER["$name/$version"]
    }
  }

  private fun lookupPluginManifest(name: String, version: String?): PactPluginManifest? {
    return if (version == null) {
      PLUGIN_MANIFEST_REGISTER.filter { it.value.name == name }.entries.maxByOrNull { it.value.version }?.value
    } else {
      PLUGIN_MANIFEST_REGISTER["$name/$version"]
    }
  }

  override fun invokeContentMatcher(
    matcher: ContentMatcher,
    expected: OptionalBody,
    actual: OptionalBody,
    allowUnexpectedKeys: Boolean,
    rules: Map<String, MatchingRuleGroup>,
    pluginConfiguration: Map<String, PluginConfiguration>
  ): Plugin.CompareContentsResponse {
    logger.debug { "invokeContentMatcher: pluginConfiguration=$pluginConfiguration" }
    return when (matcher) {
      is CatalogueContentMatcher -> {
        val pluginConfig = pluginConfiguration[matcher.pluginName]
        val pluginConfigBuilder = Plugin.PluginConfiguration.newBuilder()
        if (pluginConfig != null) {
          pluginConfigBuilder.interactionConfiguration = toProtoStruct(pluginConfig.interactionConfiguration)
          pluginConfigBuilder.pactConfiguration = toProtoStruct(pluginConfig.pactConfiguration)
        }

        val request = Plugin.CompareContentsRequest.newBuilder()
          .setExpected(Plugin.Body.newBuilder().setContent(
            BytesValue.newBuilder().setValue(ByteString.copyFrom(expected.orEmpty()))))
          .setActual(Plugin.Body.newBuilder().setContent(
            BytesValue.newBuilder().setValue(ByteString.copyFrom(actual.orEmpty()))))
          .setAllowUnexpectedKeys(allowUnexpectedKeys)
          .putAllRules(rules.entries.associate { (key, rules) ->
            key to Plugin.MatchingRules.newBuilder().addAllRule(
              rules.rules.map { rule ->
                val builder = Plugin.MatchingRule.newBuilder()
                builder
                  .setType(rule.name)
                  .setValues(builder.valuesBuilder.putAllFields(rule.attributes.entries.associate {
                    it.key to jsonToValue(it.value)
                  }.toMutableMap()))
                  .build()
              }
            ).build()
          })
          .setPluginConfiguration(pluginConfigBuilder.build())
          .build()
        val plugin = lookupPlugin(matcher.pluginName, null) ?:
          throw PactPluginNotFoundException(matcher.pluginName, null)
        plugin.withGrpcStub { stub -> stub.compareContents(request) }
      }
      else -> throw RuntimeException("Mis-configured content type matcher $matcher")
    }
  }

  override fun configureContentMatcherInteraction(
    matcher: ContentMatcher,
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Result<List<InteractionContents>, String> {
    val builder = Struct.newBuilder()
    bodyConfig.forEach { (key, value) ->
      builder.putFields(key, jsonToValue(toJson(value)))
    }
    val request = Plugin.ConfigureInteractionRequest.newBuilder()
      .setContentType(contentType)
      .setContentsConfig(builder)
      .build()
    val plugin = lookupPlugin(matcher.pluginName, null) ?:
      throw PactPluginNotFoundException(matcher.pluginName, null)

    logger.debug { "Sending configureInteraction request to plugin ${plugin.manifest}" }
    val response = plugin.withGrpcStub { stub -> stub.configureInteraction(request) }
    logger.debug { "Got response: $response" }

    return if (response.error.isNotEmpty()) {
      Err(response.error)
    } else {
      val results = mutableListOf<InteractionContents>()

      val globalPluginConfig = if (response.hasPluginConfiguration()) {
        val pluginConfiguration = PluginConfiguration()

        if (response.pluginConfiguration.hasPactConfiguration()) {
          pluginConfiguration.pactConfiguration.putAll(
            structToJson(response.pluginConfiguration.pactConfiguration).asObject()!!.entries
          )
        }

        pluginConfiguration
      } else {
        PluginConfiguration()
      }

      for (interaction in response.interactionList) {
        val returnedContentType = ContentType(interaction.contents.contentType)
        val body = OptionalBody.body(
          interaction.contents.content.value.toByteArray(), returnedContentType,
          toContentTypeHint(interaction.contents.contentTypeHint)
        )
        val rules = MatchingRuleCategory("body", interaction.rulesMap.entries.associate { (key, value) ->
          key to MatchingRuleGroup(value.ruleList.map {
            MatchingRule.create(it.type, structToJson(it.values))
          }.toMutableList(), RuleLogic.AND, false)
        }.toMutableMap())
        val generators = Generators(mutableMapOf(Category.BODY to interaction.generatorsMap.mapValues {
          createGenerator(it.value.type, structToJson(it.value.values))
        }.toMutableMap()))

        val metadata = if (interaction.hasMessageMetadata()) {
          interaction.messageMetadata.fieldsMap.entries.associate { (key, value) -> key to valueToJson(value) }
        } else {
          emptyMap()
        }

        val pluginConfig = if (interaction.hasPluginConfiguration()) {
          val pluginConfiguration = globalPluginConfig.copy()

          if (interaction.pluginConfiguration.hasInteractionConfiguration()) {
            pluginConfiguration.interactionConfiguration.putAll(
              structToJson(interaction.pluginConfiguration.interactionConfiguration).asObject()!!.entries
            )
          }

          if (interaction.pluginConfiguration.hasPactConfiguration()) {
            pluginConfiguration.pactConfiguration.putAll(
              structToJson(interaction.pluginConfiguration.pactConfiguration).asObject()!!.entries
            )
          }

          pluginConfiguration
        } else {
          PluginConfiguration()
        }

        logger.debug { "body=$body" }
        logger.debug { "rules=$rules" }
        logger.debug { "generators=$generators" }
        logger.debug { "metadata=$metadata" }
        logger.debug { "pluginConfig=$pluginConfig" }

        results.add(InteractionContents(
          interaction.partName,
          body,
          rules,
          generators,
          metadata,
          pluginConfig,
          interaction.interactionMarkup,
          interaction.interactionMarkupType.name
        ))
      }

      Ok(results)
    }
  }

  private fun toContentTypeHint(override: Plugin.Body.ContentTypeHint?): ContentTypeHint {
    return when (override) {
      Plugin.Body.ContentTypeHint.TEXT -> ContentTypeHint.TEXT
      Plugin.Body.ContentTypeHint.BINARY -> ContentTypeHint.BINARY
      else -> ContentTypeHint.DEFAULT
    }
  }

  override fun generateContent(
    contentGenerator: CatalogueContentGenerator,
    contentType: ContentType,
    generators: Map<String, Generator>,
    body: OptionalBody
  ): OptionalBody {
    val plugin = lookupPlugin(contentGenerator.catalogueEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(contentGenerator.catalogueEntry.pluginName, null)
    val request = Plugin.GenerateContentRequest.newBuilder()
      .setContents(Plugin.Body.newBuilder()
        .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(body.orEmpty())))
        .setContentType(contentType.toString()))

    generators.forEach { (key, generator) ->
      val builder = Struct.newBuilder()
      generator.toMap(PactSpecVersion.V4).forEach { (key, value) ->
        builder.putFields(key, jsonToValue(toJson(value)))
      }
      val gen = Plugin.Generator.newBuilder()
        .setType(generator.type)
        .setValues(builder)
        .build()
      request.putGenerators(key, gen)
    }
    logger.debug { "Sending generateContent request to plugin ${plugin.manifest}" }
    val response = plugin.withGrpcStub { stub -> stub.generateContent(request.build()) }
    logger.debug { "Got response: $response" }
    val returnedContentType = ContentType(response.contents.contentType)
    return OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType)
  }

  override fun startMockServer(
    catalogueEntry: CatalogueEntry,
    config: MockServerConfig,
    pact: Pact
  ): MockServerDetails {
    val plugin = lookupPlugin(catalogueEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(catalogueEntry.pluginName, null)

    val writer = StringWriter()
    DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)

    val request = Plugin.StartMockServerRequest.newBuilder()
      .setPact(writer.toString())

    logger.debug { "Sending startMockServer request to plugin ${plugin.manifest}" }
    val response = plugin.withGrpcStub { stub -> stub.startMockServer(request.build()) }
    logger.debug { "Got response: $response" }

    if (response.hasError()) {
      throw PactPluginMockServerErrorException(catalogueEntry.pluginName, response.error)
    }
    val details = response.details
    return MockServerDetails(details.key, details.address, details.port, plugin)
  }

  override fun shutdownMockServer(mockServer: MockServerDetails): List<MockServerResults>? {
    val request = Plugin.ShutdownMockServerRequest.newBuilder()
      .setServerKey(mockServer.key)

    logger.debug { "Sending shutdownMockServer request to plugin ${mockServer.plugin.manifest}" }
    val response = mockServer.plugin.withGrpcStub { stub -> stub.shutdownMockServer(request.build()) }
    logger.debug { "Got response: $response" }

    return if (response.ok) null else response.resultsList.map { result ->
      MockServerResults(result.path, result.error, result.mismatchesList.map {
        MockServerMismatch(
          it.expected, it.actual, it.mismatch, it.path, it.diff
        )
      })
    }
  }

  override fun prepareValidationForInteraction(
    transportEntry: CatalogueEntry,
    pact: V4Pact,
    interaction: V4Interaction,
    config: Map<String, Any?>
  ): Result<InteractionVerificationData, String> {
    val plugin = lookupPlugin(transportEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(transportEntry.pluginName, null)

    val writer = StringWriter()
    DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)

    val request = Plugin.VerificationPreparationRequest.newBuilder()
      .setPact(writer.toString())
      .setInteractionKey(interaction.uniqueKey())
      .setConfig(mapToProtoStruct(config))

    logger.debug { "Sending prepareValidationForInteraction request to plugin ${plugin.manifest}" }
    val response = plugin.withGrpcStub { stub -> stub.prepareInteractionForVerification(request.build()) }
    logger.debug { "Got response: $response" }

    if (response.hasError()) {
      throw PactPluginValidationForInteractionException(transportEntry.pluginName, response.error)
    }

    return Ok(InteractionVerificationData(
      OptionalBody.body(response.interactionData.body.content.value.toByteArray(), ContentType(response.interactionData.body.contentType)),
      response.interactionData.metadataMap.mapValues {
        when (it.value.valueCase) {
          Plugin.MetadataValue.ValueCase.NONBINARYVALUE -> fromProtoValue(it.value.nonBinaryValue)
          Plugin.MetadataValue.ValueCase.BINARYVALUE -> it.value.binaryValue.toByteArray()
          else -> null
        }
      }
    ))
  }

  override fun verifyInteraction(
    transportEntry: CatalogueEntry,
    verificationData: InteractionVerificationData,
    config: Map<String, Any?>,
    pact: V4Pact,
    interaction: V4Interaction
  ): Result<InteractionVerificationResult, String> {
    val plugin = lookupPlugin(transportEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(transportEntry.pluginName, null)

    val writer = StringWriter()
    DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)

    val request = Plugin.VerifyInteractionRequest.newBuilder()
      .setInteractionData(Plugin.InteractionData.newBuilder()
        .setBody(Plugin.Body.newBuilder()
          .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(verificationData.requestData.value)).build())
          .setContentType(verificationData.requestData.contentType.toString())
          .build())
        .putAllMetadata(verificationData.metadata.mapValues {
          val builder = Plugin.MetadataValue.newBuilder()

          when (val value = it.value) {
            is ByteArray -> builder.binaryValue = ByteString.copyFrom(value)
            else -> builder.nonBinaryValue = toProtoValue(value)
          }

          builder.build()
        })
        .build())
      .setConfig(mapToProtoStruct(config))
      .setPact(writer.toString())
      .setInteractionKey(interaction.uniqueKey())

    logger.debug { "Sending verifyInteraction request to plugin ${plugin.manifest}" }
    val response = plugin.withGrpcStub { stub -> stub.verifyInteraction(request.build()) }
    logger.debug { "Got response: $response" }

    return if (response.hasError()) {
      Err(response.error)
    } else {
      Ok(InteractionVerificationResult(response.result.success))
    }
  }

  private fun initialisePlugin(manifest: PactPluginManifest): Result<PactPlugin, String> {
    val result = when (manifest.executableType) {
      "exec" -> startPluginProcess(manifest)
      else -> Err("Plugin executable type of ${manifest.executableType} is not supported")
    }
    return when (result) {
      is Ok -> {
        val plugin = result.value
        PLUGIN_REGISTER["${manifest.name}/${manifest.version}"] = plugin
        logger.debug { "Plugin process started OK (port = ${plugin.port}), sending init message" }
        handleWith<PactPlugin> {
          val channel = ManagedChannelBuilder.forTarget("[::1]:${plugin.port}")
            .usePlaintext()
            .build()
          val stub = newBlockingStub(channel).withCallCredentials(BearerCredentials(plugin.serverKey))
          plugin.stub = stub
          plugin.channel = channel

          initPlugin(plugin)

          plugin
        }.mapError { err ->
          logger.error(err) { "Init call to plugin ${manifest.name} failed" }
          "Init call to plugin ${manifest.name} failed: $err"
        }
      }
      is Err -> Err(result.error)
    }
  }

  fun initPlugin(plugin: PactPlugin) {
    val request = Plugin.InitPluginRequest.newBuilder()
      .setImplementation("plugin-driver-jvm")
      .setVersion(Utils.lookupVersion(PluginManager::class.java))
      .build()

    val response =  plugin.withGrpcStub { stub -> stub.initPlugin(request) }
    logger.debug { "Got init response ${response.catalogueList} from plugin ${plugin.manifest.name}" }
    CatalogueManager.registerPluginEntries(plugin.manifest.name, response.catalogueList)
    plugin.catalogueEntries = response.catalogueList

    Thread {
      publishUpdatedCatalogue()
    }.start()
  }

  private fun publishUpdatedCatalogue() {
    val requestBuilder = Plugin.Catalogue.newBuilder()
    CatalogueManager.entries().forEach { (_, entry) ->
      requestBuilder.addCatalogue(Plugin.CatalogueEntry.newBuilder()
        .setKey(entry.key)
        .setType(entry.type.toEntry())
        .putAllValues(entry.values)
        .build())
    }
    val request = requestBuilder.build()

    PLUGIN_REGISTER.forEach { (_, plugin) ->
      plugin.withGrpcStub { stub -> stub.updateCatalogue(request) }
    }
  }

  private fun startPluginProcess(
    manifest: PactPluginManifest,
    env: Map<String, String> = mapOf(),
    vararg command: String
  ): Result<PactPlugin, String> {
    logger.debug { "Starting plugin with manifest $manifest" }
    val pb = if (command.isNotEmpty()) {
      ProcessBuilder(command.asList() + manifest.pluginDir.resolve(manifest.entryPoint).toString())
    } else {
      val osName = System.getProperty("os.name")?.lowercase()
      logger.debug { "Detected OS: $osName" }
      if (manifest.entryPoints.containsKey(osName)) {
        ProcessBuilder(manifest.pluginDir.resolve(manifest.entryPoints[osName]!!).toString())
      } else if (SystemUtils.IS_OS_WINDOWS && manifest.entryPoints.containsKey("windows")) {
        ProcessBuilder(manifest.pluginDir.resolve(manifest.entryPoints["windows"]!!).toString())
      } else {
        ProcessBuilder(manifest.pluginDir.resolve(manifest.entryPoint).toString())
      }.directory(manifest.pluginDir)
    }

    val logLevel = logLevel()
    pb.environment()["LOG_LEVEL"] = logLevel
    pb.environment()["RUST_LOG"] = logLevel
    env.forEach { (k, v) -> pb.environment()[k] = v }

    val cp = ChildProcess(pb, manifest)
    return try {
      logger.debug { "Starting plugin ${manifest.name} process ${pb.command()}" }
      cp.start()
      logger.debug { "Plugin ${manifest.name} started with PID ${cp.pid}" }
      val timeout = System.getProperty("pact.plugin.loadTimeoutInMs")?.toLongOrNull() ?: 10000
      val startupInfo = cp.channel.poll(timeout, TimeUnit.MILLISECONDS)
      if (startupInfo is JsonObject) {
        Ok(DefaultPactPlugin(cp, manifest, toInteger(startupInfo["port"]), toString(startupInfo["serverKey"])!!))
      } else {
        cp.destroy()
        Err("Plugin process did not output the correct startup message in $timeout ms - got $startupInfo")
      }
    } catch (e: Exception) {
      logger.error(e) { "Plugin process did not start correctly" }
      cp.destroy()
      Err("Plugin process did not start correctly - ${e.message}")
    }
  }

  private fun logLevel() = when {
    logger.isTraceEnabled -> "trace"
    logger.isDebugEnabled -> "debug"
    logger.isInfoEnabled -> "info"
    logger.isWarnEnabled -> "warn"
    logger.isErrorEnabled -> "error"
    else -> ""
  }

  /**
   * Searches for a plugin manifest given the name and versions
   */
  fun loadPluginManifest(name: String, version: String?): Result<PactPluginManifest, String> {
    val manifest = lookupPluginManifest(name, version)
    return if (manifest != null) {
      Ok(manifest)
    } else {
      val manifestList = mutableListOf<PactPluginManifest>()
      val pluginDir = pluginInstallDirectory()
      for (file in File(pluginDir).walk()) {
        if (file.isFile && file.name == "pact-plugin.json") {
          logger.debug { "Found plugin manifest: $file" }
          val pluginJson = file.bufferedReader().use { Json.createReader(it).readObject() }
          if (pluginJson != null) {
            val plugin = DefaultPactPluginManifest.fromJson(file.parentFile, pluginJson)
            if (plugin.name == name && versionsCompatible(plugin.version, version)) {
              logger.trace { "Manifest version is ${plugin.version}" }
              manifestList.add(plugin)
            }
          }
        }
      }

      if (manifestList.isNotEmpty()) {
        manifestList.sortByDescending { it.version }
        val selectedManifest = manifestList.first()
        PLUGIN_MANIFEST_REGISTER["$name/${selectedManifest.version}"] = selectedManifest
        Ok(selectedManifest)
      } else {
        Err("No plugin with name '$name' and version '${version ?: "any"}' was found in the Pact plugin directory '$pluginDir'")
      }
    }
  }

  /**
   * If the plugin version is compatible with the given version
   */
  fun versionsCompatible(version: String, required: String?): Boolean {
    return if (required == null || required == version) {
      true
    } else {
      val pluginVersion = Semver(version, Semver.SemverType.NPM)
      pluginVersion.satisfies(">${required}")
    }
  }

  /**
   * Returns the directory where the plugins are installed
   */
  fun pluginInstallDirectory(): String {
    val pluginDirEnvVar = lookupEnvironmentValue("pact.plugin.dir")
    return if (pluginDirEnvVar.isNotEmpty()) {
      pluginDirEnvVar!!
    } else if (System.getProperty("user.home").isNotEmpty()) {
      System.getProperty("user.home") + "/.pact/plugins"
    } else {
      System.getenv("HOME") + "/.pact/plugins"
    }
  }
}

class BearerCredentials(private val serverKey: String?) : CallCredentials() {
  override fun applyRequestMetadata(
    requestInfo: RequestInfo,
    appExecutor: Executor,
    applier: MetadataApplier
  ) {
    if (serverKey.isNotEmpty()) {
      val metadata = Metadata()
      metadata.put(Metadata.Key.of("authorization", Metadata.ASCII_STRING_MARSHALLER), serverKey)
      applier.apply(metadata)
    }
  }

  override fun thisUsesUnstableApi() {}
}

/**
 * Plugin configuration to add to the matching context for an interaction
 */
data class PluginConfiguration(
  val interactionConfiguration: MutableMap<String, JsonValue> = mutableMapOf(),
  val pactConfiguration: MutableMap<String, JsonValue> = mutableMapOf()
)

/**
 * Interaction contents returned from the plugin
 */
data class InteractionContents @JvmOverloads constructor(
  /**
   * The part that the contents are for (like request or response). Only used if there are multiple.
   */
  val partName: String,

  /**
   * Body for the contents
   */
  val body: OptionalBody,

  /**
   * Matching rules to apply
   */
  val rules: MatchingRuleCategory? = null,

  /**
   * Generators to apply
   */
  val generators: Generators? = null,

  /**
   * Metadata for the contents. This is only applied to messages.
   */
  val metadata: Map<String, JsonValue> = emptyMap(),

  /**
   * Any plugin specific data to store with the interaction
   */
  val pluginConfig: PluginConfiguration = PluginConfiguration(),

  /**
   * Markup to use to display the interaction in user interfaces
   */
  val interactionMarkup: String = "",

  /**
   * The type of the markup. Defaults to CommonMark.
   */
  val interactionMarkupType: String = ""
)
