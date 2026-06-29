package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import au.com.dius.pact.core.model.ContentTypeHint
import au.com.dius.pact.core.model.DefaultPactWriter
import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.Pact
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.PluginData
import au.com.dius.pact.core.model.V4Interaction
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.model.v4.V4InteractionType
import au.com.dius.pact.core.model.generators.Category
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.generators.GeneratorTestMode
import au.com.dius.pact.core.model.generators.Generators
import au.com.dius.pact.core.model.generators.createGenerator
import au.com.dius.pact.core.model.matchingrules.MatchingRule
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import au.com.dius.pact.core.model.matchingrules.RuleLogic
import au.com.dius.pact.core.support.Json.toJson
import au.com.dius.pact.core.support.Result
import au.com.dius.pact.core.support.Utils.lookupEnvironmentValue
import au.com.dius.pact.core.support.isNotEmpty
import au.com.dius.pact.core.support.json.JsonParser
import au.com.dius.pact.core.support.json.JsonValue
import au.com.dius.pact.core.support.mapError
import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.Struct
import com.vdurmont.semver4j.Semver
import io.github.oshai.kotlinlogging.KotlinLogging
import io.grpc.CallCredentials
import io.grpc.ManagedChannel
import io.grpc.ManagedChannelBuilder
import io.grpc.Metadata
import io.pact.plugin.PactPluginGrpc.newBlockingStub
import io.pact.plugin.v2.PactPluginGrpc.newBlockingStub as newV2BlockingStub
import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import io.pact.plugins.jvm.core.Utils.fromProtoValue
import io.pact.plugins.jvm.core.Utils.jsonToValue
import io.pact.plugins.jvm.core.Utils.mapToProtoStruct
import io.pact.plugins.jvm.core.Utils.structToJson
import io.pact.plugins.jvm.core.Utils.toProtoStruct
import io.pact.plugins.jvm.core.Utils.toProtoValue
import io.pact.plugins.jvm.core.Utils.valueToJson
import org.apache.commons.lang3.SystemUtils
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.lang.Runtime.getRuntime
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit

private val logger = KotlinLogging.logger {}


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
   * Parameters to pass into the command line
   */
  val args: List<String>

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
  override val args: List<String>,
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

    if (args.isNotEmpty()) {
      map["args"] = args
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

  companion object {
    @JvmStatic
    fun fromJson(pluginDir: File, pluginJson: JsonValue): PactPluginManifest {
      val entryPoints = if (pluginJson.has("entryPoints")) {
        when (val ep = pluginJson["entryPoints"]) {
          is JsonValue.Object -> ep.entries.entries.associate { it.key to it.value.toString() }
          else -> {
            logger.warn { "entryPoints field in plugin manifest is invalid" }
            emptyMap()
          }
        }
      } else {
        emptyMap()
      }

      val args = if (pluginJson.has("args")) {
        when (val aj = pluginJson["args"]) {
          is JsonValue.Array -> aj.values.map { it.toString() }
          else -> {
            logger.warn { "args field in plugin manifest is invalid" }
            emptyList()
          }
        }
      } else {
        emptyList()
      }

      return DefaultPactPluginManifest(
        pluginDir,
        toInteger(pluginJson["pluginInterfaceVersion"]) ?: 1,
        pluginJson["name"].asString().orEmpty(),
        pluginJson["version"].asString().orEmpty(),
        pluginJson["executableType"].asString().orEmpty(),
        pluginJson["minimumRequiredVersion"].asString(),
        pluginJson["entryPoint"].asString().orEmpty(),
        entryPoints,
        args,
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
  /** UUID assigned by the driver at process start; included in every log record emitted by this instance */
  val instanceId: String
  var catalogueEntries: List<Plugin.CatalogueEntry>?
  var pluginCapabilities: List<String>

  /**
   * Shutdown the running plugin
   */
  fun shutdown()

  /**
   * Invoke the callback with the version-specific RPC client connected to the running plugin.
   */
  fun <T> withRpcClient(callback: java.util.function.Function<PactPluginRpcClient, T>): T
}

/**
 * Default implementation of a Pact Plugin
 */
data class DefaultPactPlugin(
  val cp: ChildProcess,
  override val manifest: PactPluginManifest,
  override val port: Int?,
  override val serverKey: String,
  override val instanceId: String,
  var rpcClient: PactPluginRpcClient? = null,
  override var catalogueEntries: List<Plugin.CatalogueEntry>? = null,
  override var pluginCapabilities: List<String> = emptyList(),
  var channel: ManagedChannel? = null
) : PactPlugin {
  override val processPid: Long
    get() = cp.pid

  override fun shutdown() {
    PluginHostServer.deregisterInstance(instanceId)
    cp.destroy()
    if (channel != null) {
      channel!!.shutdownNow().awaitTermination(1, TimeUnit.SECONDS)
    }
  }

  override fun <T> withRpcClient(callback: java.util.function.Function<PactPluginRpcClient, T>): T {
    return callback.apply(requireNotNull(rpcClient) { "Plugin RPC client has not been initialised" })
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
    body: OptionalBody,
    testMode: GeneratorTestMode,
    pluginData: List<PluginData>,
    interactionData: Map<String, Map<String, JsonValue>>,
    testContext: Map<String, JsonValue>,
    forRequest: Boolean
  ): OptionalBody

  /**
   * Starts a mock server given the catalog entry for it and a Pact
   */
  fun startMockServer(
    catalogueEntry: CatalogueEntry,
    config: MockServerConfig,
    pact: Pact
  ): MockServerDetails

  /**
   * Starts a mock server given the catalog entry for it and a Pact
   */
  fun startMockServer(
    catalogueEntry: CatalogueEntry,
    config: MockServerConfig,
    pact: Pact,
    testContext: Map<String, JsonValue>,
  ): MockServerDetails

  /**
   * Shutdowns a running mock server. Will return any errors from the mock server.
   */
  fun shutdownMockServer(mockServer: MockServerDetails): List<MockServerResults>?

  /**
   * Gets the results from a running mock server.
   */
  fun getMockServerResults(mockServer: MockServerDetails): List<MockServerResults>?

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

object DefaultPluginManager: PluginManager {
  private val PLUGIN_MANIFEST_REGISTER: MutableMap<String, PactPluginManifest> = mutableMapOf()
  private val PLUGIN_REGISTER: MutableMap<String, PactPlugin> = ConcurrentHashMap()

  private var pluginDownloader: PluginDownloader = DefaultPluginDownloader
  private var repository: Repository = DefaultRepository()

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
        Result.Ok(plugin)
      } else {
        val pluginManifest = when (val manifest = loadPluginManifest(name, version)) {
          is Result.Ok -> manifest
          is Result.Err -> {
            logger.warn { "Could not load plugin manifest from disk, will try auto install it: ${manifest.error}" }
            when (val index = repository.fetchRepositoryIndex()) {
              is Result.Ok -> {
                val pluginVersion = index.value.lookupPluginVersion(name, version)
                if (pluginVersion != null) {
                  logger.info { "Found an entry for the plugin in the plugin index, will try install that" }
                  when (val installed = pluginDownloader.installPluginFromUrl(pluginVersion.source.value)) {
                    is Result.Ok -> {
                      PLUGIN_MANIFEST_REGISTER["$name/${installed.value.version}"] = installed.value
                      installed
                    }
                    is Result.Err -> installed
                  }
                } else {
                  Result.Err(manifest.error)
                }
              }
              is Result.Err -> Result.Err(index.error)
            }
          }
        }

        when (pluginManifest) {
          is Result.Ok -> {
            PluginMetrics.sendMetrics(pluginManifest.value)
            initialisePlugin(pluginManifest.value)
          }
          is Result.Err -> pluginManifest
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

        val expectedContent = Plugin.Body.newBuilder()
          .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(expected.orEmpty())))
          .setContentType(expected.contentType.toString())
          .setContentTypeHint(toInterfaceType(expected.contentTypeHint))
        val actualContent = Plugin.Body.newBuilder()
          .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(actual.orEmpty())))
          .setContentType(actual.contentType.toString())
          .setContentTypeHint(toInterfaceType(actual.contentTypeHint))
        val request = Plugin.CompareContentsRequest.newBuilder()
          .setExpected(expectedContent)
          .setActual(actualContent)
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
        plugin.withRpcClient { client -> client.compareContents(request) }
      }
      else -> throw RuntimeException("Mis-configured content type matcher $matcher")
    }
  }

  private fun toInterfaceType(contentTypeHint: ContentTypeHint): Plugin.Body.ContentTypeHint {
    return when (contentTypeHint) {
      ContentTypeHint.BINARY -> Plugin.Body.ContentTypeHint.BINARY
      ContentTypeHint.TEXT -> Plugin.Body.ContentTypeHint.TEXT
      ContentTypeHint.DEFAULT -> Plugin.Body.ContentTypeHint.DEFAULT
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
    val response = plugin.withRpcClient { client -> client.configureInteraction(request) }
    logger.debug { "Got response: $response" }

    return if (response.error.isNotEmpty()) {
      Result.Err(response.error)
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

        val rules = if (interaction.rulesCount > 0)
          MatchingRuleCategory("body", interaction.rulesMap.entries.associate { (key, value) ->
            key to MatchingRuleGroup(value.ruleList.map {
              MatchingRule.create(it.type, structToJson(it.values))
            }.toMutableList(), RuleLogic.AND, false)
          }.toMutableMap())
        else null

        val metadata = if (interaction.hasMessageMetadata()) {
          interaction.messageMetadata.fieldsMap.entries.associate { (key, value) -> key to valueToJson(value) }
        } else {
          emptyMap()
        }

        val metadataRules = if (interaction.metadataRulesCount > 0)
          MatchingRuleCategory("metadata", interaction.metadataRulesMap.entries.associate { (key, value) ->
            key to MatchingRuleGroup(value.ruleList.map {
              MatchingRule.create(it.type, structToJson(it.values))
            }.toMutableList(), RuleLogic.AND, false)
          }.toMutableMap())
        else null

        val categories = mutableMapOf<Category, MutableMap<String, Generator>>()
        if (interaction.generatorsCount > 0) {
          categories[Category.BODY] = interaction.generatorsMap.mapValues {
            createGenerator(it.value.type, structToJson(it.value.values))
          }.toMutableMap()
        }
        if (interaction.metadataGeneratorsCount > 0) {
          categories[Category.METADATA] = interaction.metadataGeneratorsMap.mapValues {
            createGenerator(it.value.type, structToJson(it.value.values))
          }.toMutableMap()
        }
        val generators = Generators(categories)

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
        logger.debug { "metadataRules=$metadataRules" }
        logger.debug { "pluginConfig=$pluginConfig" }

        results.add(InteractionContents(
          interaction.partName,
          body,
          rules,
          generators,
          metadata,
          pluginConfig,
          interaction.interactionMarkup,
          interaction.interactionMarkupType.name,
          metadataRules
        ))
      }

      Result.Ok(results)
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
    body: OptionalBody,
    testMode: GeneratorTestMode,
    pluginData: List<PluginData>,
    interactionData: Map<String, Map<String, JsonValue>>,
    testContext: Map<String, JsonValue>,
    forRequest: Boolean
  ): OptionalBody {
    val plugin = lookupPlugin(contentGenerator.catalogueEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(contentGenerator.catalogueEntry.pluginName, null)

    val pluginConfig = pluginData.find { it.name == plugin.manifest.name }?.configuration?.mapValues {
      toJson(it.value)
    }
    val interactionConfig = interactionData[plugin.manifest.name]
    val pluginConfigBuilder = Plugin.PluginConfiguration.newBuilder()
    if (!pluginConfig.isNullOrEmpty()) {
      pluginConfigBuilder.pactConfiguration = toProtoStruct(pluginConfig)
    }
    if (!interactionConfig.isNullOrEmpty()) {
      pluginConfigBuilder.interactionConfiguration = toProtoStruct(interactionConfig)
    }

    val request = Plugin.GenerateContentRequest.newBuilder()
      .setContents(Plugin.Body.newBuilder()
        .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(body.orEmpty())))
        .setContentType(contentType.toString()))
      .setPluginConfiguration(pluginConfigBuilder)
      .setTestContext(mapToProtoStruct(testContext))
      .setTestMode(if (testMode == GeneratorTestMode.Consumer)
        Plugin.GenerateContentRequest.TestMode.Consumer
        else Plugin.GenerateContentRequest.TestMode.Provider)
      .setContentFor(if (forRequest) Plugin.GenerateContentRequest.ContentFor.Request
        else Plugin.GenerateContentRequest.ContentFor.Response)

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
    val response = plugin.withRpcClient { client -> client.generateContent(request.build()) }
    logger.debug { "Got response: $response" }
    val returnedContentType = ContentType(response.contents.contentType)
    return OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType)
  }

  private fun buildInteractionContents(
    pluginName: String,
    pact: V4Pact,
    interaction: V4Interaction
  ): PluginV2.InteractionContents {
    val interactionType = when {
      interaction.isInteractionType(V4InteractionType.SynchronousHTTP) -> V4InteractionType.SynchronousHTTP.toString()
      interaction.isInteractionType(V4InteractionType.AsynchronousMessages) -> V4InteractionType.AsynchronousMessages.toString()
      interaction.isInteractionType(V4InteractionType.SynchronousMessages) -> V4InteractionType.SynchronousMessages.toString()
      else -> ""
    }

    val pluginConfigBuilder = PluginV2.PluginConfiguration.newBuilder()
    val interactionConfig = interaction.pluginConfiguration[pluginName]
    if (!interactionConfig.isNullOrEmpty()) {
      pluginConfigBuilder.interactionConfiguration = toProtoStruct(interactionConfig)
    }
    val pactConfig = pact.pluginData().find { it.name == pluginName }?.configAsJsonMap()
    if (!pactConfig.isNullOrEmpty()) {
      val pactConfiguration = pactConfig["pactConfiguration"]?.asObject()?.entries
      if (!pactConfiguration.isNullOrEmpty()) {
        pluginConfigBuilder.pactConfiguration = toProtoStruct(pactConfiguration)
      }
    }

    return PluginV2.InteractionContents.newBuilder()
      .setInteractionType(interactionType)
      .setPluginConfiguration(pluginConfigBuilder.build())
      .setConsumer(pact.consumer.name)
      .setProvider(pact.provider.name)
      .build()
  }

  override fun startMockServer(
    catalogueEntry: CatalogueEntry,
    config: MockServerConfig,
    pact: Pact
  ) = startMockServer(catalogueEntry, config, pact, emptyMap())

  override fun startMockServer(
    catalogueEntry: CatalogueEntry,
    config: MockServerConfig,
    pact: Pact,
    testContext: Map<String, JsonValue>
  ): MockServerDetails {
    val plugin = lookupPlugin(catalogueEntry.pluginName, null) ?:
      throw PactPluginNotFoundException(catalogueEntry.pluginName, null)

    logger.debug { "Sending startMockServer request to plugin ${plugin.manifest}" }
    val response = if (plugin.manifest.pluginInterfaceVersion >= 2) {
      val v4Pact = pact as? V4Pact
        ?: throw PactPluginMockServerErrorException(catalogueEntry.pluginName, "V2 plugins require a V4 pact")
      val interactions = v4Pact.interactions.filterIsInstance<V4Interaction>().map { interaction ->
        buildInteractionContents(plugin.manifest.name, v4Pact, interaction)
      }
      val v2Request = PluginV2.StartMockServerRequest.newBuilder()
        .addAllInteractions(interactions)
        .setTestContext(mapToProtoStruct(testContext))
      if (config.hostInterface.isNotEmpty()) {
        v2Request.hostInterface = config.hostInterface
      }
      v2Request.port = config.port
      v2Request.tls = config.tls
      plugin.withRpcClient { client -> client.startMockServerV2(v2Request.build()) }
    } else {
      val writer = StringWriter()
      DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)
      val request = Plugin.StartMockServerRequest.newBuilder()
        .setPact(writer.toString())
        .setTestContext(mapToProtoStruct(testContext))
      if (config.hostInterface.isNotEmpty()) {
        request.hostInterface = config.hostInterface
      }
      request.port = config.port
      request.tls = config.tls
      plugin.withRpcClient { client -> client.startMockServer(request.build()) }
    }
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
    val response = mockServer.plugin.withRpcClient { client -> client.shutdownMockServer(request.build()) }
    logger.debug { "Got response: $response" }

    return if (response.ok) null else response.resultsList.map { result ->
      MockServerResults(result.path, result.error, result.mismatchesList.map {
        MockServerMismatch(
          it.expected, it.actual, it.mismatch, it.path, it.diff, it.mismatchType
        )
      })
    }
  }

  override fun getMockServerResults(mockServer: MockServerDetails): List<MockServerResults>? {
    val request = Plugin.MockServerRequest.newBuilder()
            .setServerKey(mockServer.key)

    logger.debug { "Sending getMockServerResults request to plugin ${mockServer.plugin.manifest}" }
    val response = mockServer.plugin.withRpcClient { client -> client.getMockServerResults(request.build()) }
    logger.debug { "Got response: $response" }

    return if (response.ok) null else response.resultsList.map { result ->
      MockServerResults(result.path, result.error, result.mismatchesList.map {
        MockServerMismatch(
          it.expected, it.actual, it.mismatch, it.path, it.diff, it.mismatchType
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

    logger.debug { "Sending prepareValidationForInteraction request to plugin ${plugin.manifest}" }
    val response = if (plugin.manifest.pluginInterfaceVersion >= 2) {
      val v2Request = PluginV2.VerificationPreparationRequest.newBuilder()
        .setInteractionContents(buildInteractionContents(plugin.manifest.name, pact, interaction))
        .setConfig(mapToProtoStruct(config))
        .build()
      plugin.withRpcClient { client -> client.prepareInteractionForVerificationV2(v2Request) }
    } else {
      val writer = StringWriter()
      DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)
      val request = Plugin.VerificationPreparationRequest.newBuilder()
        .setPact(writer.toString())
        .setInteractionKey(interaction.uniqueKey())
        .setConfig(mapToProtoStruct(config))
        .build()
      plugin.withRpcClient { client -> client.prepareInteractionForVerification(request) }
    }
    logger.debug { "Got response: $response" }

    if (response.hasError()) {
      throw PactPluginValidationForInteractionException(transportEntry.pluginName, response.error)
    }

    return Result.Ok(InteractionVerificationData(
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

    val interactionDataV1 = Plugin.InteractionData.newBuilder()
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
      .build()

    logger.debug { "Sending verifyInteraction request to plugin ${plugin.manifest}" }
    val response = if (plugin.manifest.pluginInterfaceVersion >= 2) {
      val v2InteractionData = PluginV2.InteractionData.parser().parseFrom(interactionDataV1.toByteArray())
      val v2Request = PluginV2.VerifyInteractionRequest.newBuilder()
        .setInteractionData(v2InteractionData)
        .setConfig(mapToProtoStruct(config))
        .setInteractionContents(buildInteractionContents(plugin.manifest.name, pact, interaction))
        .build()
      plugin.withRpcClient { client -> client.verifyInteractionV2(v2Request) }
    } else {
      val writer = StringWriter()
      DefaultPactWriter.writePact(pact, PrintWriter(writer), PactSpecVersion.V4)
      val request = Plugin.VerifyInteractionRequest.newBuilder()
        .setInteractionData(interactionDataV1)
        .setConfig(mapToProtoStruct(config))
        .setPact(writer.toString())
        .setInteractionKey(interaction.uniqueKey())
        .build()
      plugin.withRpcClient { client -> client.verifyInteraction(request) }
    }
    logger.debug { "Got response: $response" }

    return if (response.hasError()) {
      Result.Err(response.error)
    } else {
      Result.Ok(InteractionVerificationResult(response.result.success, response.result.mismatchesList.map {
        if (it.hasError()) {
          InteractionVerificationDetails.Error(it.error)
        } else {
          InteractionVerificationDetails.Mismatch(it.mismatch.expected.value.toByteArray(),
            it.mismatch.actual.value.toByteArray(), it.mismatch.mismatch, it.mismatch.path)
        }
      }, response.result.outputList))
    }
  }

  private fun initialisePlugin(manifest: PactPluginManifest): Result<PactPlugin, String> {
    val interfaceVersion = PluginInterfaceVersion.from(manifest.pluginInterfaceVersion)
      ?: return Result.Err("Unsupported plugin interface version ${manifest.pluginInterfaceVersion} for ${manifest.name}")

    val result = when (manifest.executableType) {
      "exec" -> startPluginProcess(manifest)
      else -> Result.Err("Plugin executable type of ${manifest.executableType} is not supported")
    }
    return when (result) {
      is Result.Ok -> {
        val plugin = result.value
        PLUGIN_REGISTER["${manifest.name}/${manifest.version}"] = plugin
        logger.debug { "Plugin process started OK (port = ${plugin.port}), sending init message" }

        val initResult = tryInitPlugin(plugin, "[::1]:${plugin.port}")
        when (initResult) {
          is Result.Ok -> initResult
          is Result.Err -> {
            logger.debug { "Init call to plugin ${manifest.name} failed, will try an IP4 address" }
            tryInitPlugin(plugin, "127.0.0.1:${plugin.port}").mapError { err ->
              logger.error(err) { "Init call to plugin ${manifest.name} failed" }
              "Init call to plugin ${manifest.name} failed: $err"
            }
          }
        }
      }
      is Result.Err -> Result.Err(result.error)
    }
  }

  private fun tryInitPlugin(plugin: DefaultPactPlugin, address: String): Result<PactPlugin, Exception> {
    try {
      val channel = ManagedChannelBuilder.forTarget(address)
        .usePlaintext()
        .build()
      val interfaceVersion = requireNotNull(PluginInterfaceVersion.from(plugin.manifest.pluginInterfaceVersion)) {
        "Plugin ${plugin.manifest.name} has unsupported interface version " +
          "${plugin.manifest.pluginInterfaceVersion} — should have been caught in initialisePlugin"
      }
      val rpcClient = when (interfaceVersion) {
        PluginInterfaceVersion.V1 -> PactPluginV1RpcClient(
          newBlockingStub(channel).withCallCredentials(BearerCredentials(plugin.serverKey))
        )
        PluginInterfaceVersion.V2 -> PactPluginV2RpcClient(
          newV2BlockingStub(channel).withCallCredentials(BearerCredentials(plugin.serverKey))
        )
      }
      plugin.rpcClient = rpcClient
      plugin.channel = channel

      try {
        initPlugin(plugin)
      } catch (e: Exception) {
        plugin.rpcClient = null
        plugin.channel = null
        channel.shutdownNow()
        throw e
      }

      return Result.Ok(plugin)
    } catch (e: Exception) {
      return Result.Err(e)
    }
  }

  fun initPlugin(plugin: PactPlugin) {
    val request = PluginInitRequest(
      implementation = "plugin-driver-jvm",
      version = Utils.lookupVersion(PluginManager::class.java),
      hostCapabilities = CatalogueManager.coreEntries().map { "${it.type}/${it.key}" },
      pluginInstanceId = plugin.instanceId
    )

    val response =  plugin.withRpcClient { client -> client.initPlugin(request) }
    logger.debug { "Got init response $response from plugin ${plugin.manifest.name}" }
    CatalogueManager.registerPluginEntries(plugin.manifest.name, response.catalogueEntries)
    plugin.catalogueEntries = response.catalogueEntries
    plugin.pluginCapabilities = response.pluginCapabilities

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
      plugin.withRpcClient { client -> client.updateCatalogue(request) }
    }
  }

  private fun startPluginProcess(
    manifest: PactPluginManifest,
    env: Map<String, String> = mapOf(),
    vararg command: String
  ): Result<DefaultPactPlugin, String> {
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
    val instanceId = java.util.UUID.randomUUID().toString()
    logger.debug { "Plugin ${manifest.name} assigned instance ID $instanceId" }
    pb.environment()["PACT_PLUGIN_INSTANCE_ID"] = instanceId

    try {
      val hostPort = PluginHostServer.ensureRunning()
      pb.environment()["PACT_PLUGIN_HOST"] = "127.0.0.1:$hostPort"
    } catch (e: Exception) {
      logger.warn(e) { "Could not start PluginHost server, Log RPC forwarding will be unavailable" }
    }
    env.forEach { (k, v) -> pb.environment()[k] = v }

    if (manifest.args.isNotEmpty()) {
      pb.command().addAll(manifest.args)
    }

    val cp = ChildProcess(pb, manifest, instanceId)
    return try {
      logger.debug { "Starting plugin ${manifest.name} process ${pb.command()}" }
      cp.start()
      PluginHostServer.registerInstance(instanceId, manifest.name)
      logger.debug { "Plugin ${manifest.name} started with PID ${cp.pid}" }
      val timeout = System.getProperty("pact.plugin.loadTimeoutInMs")?.toLongOrNull() ?: 10000
      val startupInfo = cp.channel.poll(timeout, TimeUnit.MILLISECONDS)
      if (startupInfo is JsonValue.Object) {
        val plugin = DefaultPactPlugin(cp, manifest, toInteger(startupInfo["port"]), startupInfo["serverKey"].toString(), instanceId)
        Result.Ok(plugin)
      } else {
        cp.destroy()
        PluginHostServer.deregisterInstance(instanceId)
        Result.Err("Plugin process did not output the correct startup message in $timeout ms - got $startupInfo")
      }
    } catch (e: Exception) {
      logger.error(e) { "Plugin process did not start correctly" }
      cp.destroy()
      PluginHostServer.deregisterInstance(instanceId)
      Result.Err("Plugin process did not start correctly - ${e.message}")
    }
  }

  private fun logLevel() = when {
    logger.isTraceEnabled() -> "trace"
    logger.isDebugEnabled() -> "debug"
    logger.isInfoEnabled() -> "info"
    logger.isWarnEnabled() -> "warn"
    logger.isErrorEnabled() -> "error"
    else -> ""
  }

  /**
   * Searches for a plugin manifest given the name and versions
   */
  fun loadPluginManifest(name: String, version: String?): Result<PactPluginManifest, String> {
    val manifest = lookupPluginManifest(name, version)
    return if (manifest != null) {
      Result.Ok(manifest)
    } else {
      val manifestList = mutableListOf<PactPluginManifest>()
      val pluginDir = pluginInstallDirectory()
      for (file in File(pluginDir).walk()) {
        if (file.isFile && file.name == "pact-plugin.json") {
          logger.debug { "Found plugin manifest: $file" }
          val pluginJson = file.bufferedReader().use { JsonParser.parseReader(it).asObject() }
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
        val selectedManifest = maxVersion(manifestList)!!
        PLUGIN_MANIFEST_REGISTER["$name/${selectedManifest.version}"] = selectedManifest
        Result.Ok(selectedManifest)
      } else {
        Result.Err("No plugin with name '$name' and version '${version ?: "any"}' was found in the Pact plugin directory '$pluginDir'")
      }
    }
  }

  /**
   * Return the max valid version of the found plugins
   */
  fun maxVersion(manifestList: List<PactPluginManifest>): PactPluginManifest? {
    return manifestList.sortedWith { a, b ->
      val versionA = Semver(a.version, Semver.SemverType.STRICT)
      val versionB = Semver(b.version, Semver.SemverType.STRICT)
      versionA.compareTo(versionB)
    }.lastOrNull()
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
  val interactionMarkupType: String = "",

  /**
   * Matching rules to apply to any message metadata
   */
  val metadataRules: MatchingRuleCategory? = null
)
