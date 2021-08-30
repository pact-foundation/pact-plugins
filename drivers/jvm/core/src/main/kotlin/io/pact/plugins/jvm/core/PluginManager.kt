package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.ContentType
import au.com.dius.pact.core.model.ContentTypeOverride
import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.generators.Category
import au.com.dius.pact.core.model.generators.Generator
import au.com.dius.pact.core.model.generators.Generators
import au.com.dius.pact.core.model.generators.createGenerator
import au.com.dius.pact.core.model.matchingrules.MatchingRule
import au.com.dius.pact.core.model.matchingrules.MatchingRuleCategory
import au.com.dius.pact.core.model.matchingrules.MatchingRuleGroup
import au.com.dius.pact.core.model.matchingrules.RuleLogic
import au.com.dius.pact.core.support.Json.toJson
import au.com.dius.pact.core.support.json.JsonValue
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.mapError
import com.google.protobuf.ByteString
import com.google.protobuf.BytesValue
import com.google.protobuf.Struct
import com.vdurmont.semver4j.Semver
import io.grpc.ManagedChannel
import io.grpc.ManagedChannelBuilder
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.PactPluginGrpc.newBlockingStub
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.Utils.handleWith
import io.pact.plugins.jvm.core.Utils.jsonToValue
import io.pact.plugins.jvm.core.Utils.structToJson
import io.pact.plugins.jvm.core.Utils.valueToJson
import mu.KLogging
import org.apache.commons.lang3.SystemUtils
import java.io.BufferedReader
import java.io.File
import java.io.IOException
import java.io.InputStreamReader
import java.lang.Runtime.getRuntime
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import javax.json.Json
import javax.json.JsonObject

enum class PluginDependencyType {
  OSPackage, Plugin, Library, Executable
}

data class PluginDependency(
  val name: String,
  val version: String?,
  val type: PluginDependencyType
)

interface PactPluginManifest {
  val pluginDir: File
  val pluginInterfaceVersion: Int
  val name: String
  val version: String
  val executableType: String
  val minimumRequiredVersion: String?
  val entryPoint: String
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
    fun fromJson(pluginDir: File, pluginJson: JsonObject): PactPluginManifest {
      return DefaultPactPluginManifest(
        pluginDir,
        toInteger(pluginJson["pluginInterfaceVersion"]) ?: 1,
        toString(pluginJson["name"])!!,
        toString(pluginJson["version"])!!,
        toString(pluginJson["executableType"])!!,
        toString(pluginJson["minimumRequiredVersion"]),
        toString(pluginJson["entryPoint"])!!,
        listOf()
      )
    }
  }
}

interface PactPlugin {
  val manifest: PactPluginManifest
  val port: Int?
  val serverKey: String?
  val processPid: Long?
  var stub: PactPluginGrpc.PactPluginBlockingStub?
  var catalogueEntries: List<Plugin.CatalogueEntry>?
  var channel: ManagedChannel?

  fun shutdown()
}

data class DefaultPactPlugin(
  val cp: ChildProcess,
  override val manifest: PactPluginManifest,
  override val port: Int?,
  override val serverKey: String,
  override var stub: PactPluginGrpc.PactPluginBlockingStub? = null,
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
}

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
    rules: Map<String, MatchingRuleGroup>
  ): Plugin.CompareContentsResponse


  /**
   * Invoke the content type matcher to configure the interaction
   */
  fun configureContentMatcherInteraction(
    matcher: ContentMatcher,
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): Any?

  /**
   * Invoke the content generator to generate the contents for a body
   */
  fun generateContent(
    contentGenerator: CatalogueContentGenerator,
    contentType: ContentType,
    generators: Map<String, Generator>,
    body: OptionalBody
  ): OptionalBody
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
    val plugin = lookupPlugin(name, version)
    return if (plugin != null) {
      PluginMetrics.sendMetrics(plugin.manifest)
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
    rules: Map<String, MatchingRuleGroup>
  ): Plugin.CompareContentsResponse {
    return when (matcher) {
      is CatalogueContentMatcher -> {
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
          .build()
        val plugin = lookupPlugin(matcher.pluginName, null) ?:
          throw PactPluginNotFoundException(matcher.pluginName, null)
        plugin.stub!!.compareContents(request)
      }
      else -> throw RuntimeException("Mis-configured content type matcher $matcher")
    }
  }

  override fun configureContentMatcherInteraction(
    matcher: ContentMatcher,
    contentType: String,
    bodyConfig: Map<String, Any?>
  ): InteractionContents {
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
    val response = plugin.stub!!.configureInteraction(request)
    logger.debug { "Got response: $response" }
    val returnedContentType = ContentType(response.contents.contentType)
    val body = OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType,
      toContentTypeOverride(response.contents.contentTypeOverride))
    val rules = MatchingRuleCategory("body", response.rulesMap.entries.associate { (key, value) ->
      key to MatchingRuleGroup(value.ruleList.map {
        MatchingRule.create(it.type, structToJson(it.values))
      }.toMutableList(), RuleLogic.AND, false)
    }.toMutableMap())
    val generators = Generators(mutableMapOf(Category.BODY to response.generatorsMap.mapValues {
      createGenerator(it.value.type, structToJson(it.value.values))
    }.toMutableMap()))

    val metadata = if (response.hasMessageMetadata()) {
       response.messageMetadata.fieldsMap.entries.associate { (key, value) -> key to valueToJson(value) }
    } else {
      emptyMap()
    }

    val pluginConfig = if (response.hasPluginConfiguration()) {
      val pluginConfiguration = PluginConfiguration()

      if (response.pluginConfiguration.hasInteractionConfiguration()) {
        pluginConfiguration.interactionConfiguration.putAll(
          structToJson(response.pluginConfiguration.interactionConfiguration).asObject()!!.entries
        )
      }
      if (response.pluginConfiguration.hasPactConfiguration()) {
        pluginConfiguration.pactConfiguration.putAll(
          structToJson(response.pluginConfiguration.pactConfiguration).asObject()!!.entries
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
    return InteractionContents(body, rules, generators, metadata, pluginConfig)
  }

  private fun toContentTypeOverride(override: Plugin.Body.ContentTypeOverride?): ContentTypeOverride {
    return when (override) {
      Plugin.Body.ContentTypeOverride.TEXT -> ContentTypeOverride.TEXT
      Plugin.Body.ContentTypeOverride.BINARY -> ContentTypeOverride.BINARY
      else -> ContentTypeOverride.DEFAULT
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
    val response = plugin.stub!!.generateContent(request.build())
    logger.debug { "Got response: $response" }
    val returnedContentType = ContentType(response.contents.contentType)
    return OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType)
  }

  private fun initialisePlugin(manifest: PactPluginManifest): Result<PactPlugin, String> {
    val result = when (manifest.executableType) {
      "exec" -> startPluginProcess(manifest)
      "ruby" -> loadRubyPlugin(manifest)
      else -> Err("Plugin executable type of ${manifest.executableType} is not supported")
    }
    return when (result) {
      is Ok -> {
        val plugin = result.value
        PLUGIN_REGISTER["${manifest.name}/${manifest.version}"] = plugin
        logger.debug { "Plugin process started OK (port = ${plugin.port}), sending init message" }
        handleWith<PactPlugin> {
          val channel = ManagedChannelBuilder.forTarget("127.0.0.1:${plugin.port}")
            .usePlaintext()
            .build()
          val stub = newBlockingStub(channel)
          val response = makeInitRequest(stub)
          logger.debug { "Got init response ${response.catalogueList} from plugin ${manifest.name}" }
          CatalogueManager.registerPluginEntries(manifest.name, response.catalogueList)
          plugin.stub = stub
          plugin.channel = channel
          plugin.catalogueEntries = response.catalogueList
          Thread {
            publishUpdatedCatalogue()
          }.run()
          plugin
        }.mapError { err ->
          logger.error(err) { "Init call to plugin ${manifest.name} failed" }
          "Init call to plugin ${manifest.name} failed: $err"
        }
      }
      is Err -> Err(result.error)
    }
  }

  fun makeInitRequest(stub: PactPluginGrpc.PactPluginBlockingStub): Plugin.InitPluginResponse {
    val request = Plugin.InitPluginRequest.newBuilder()
      .setImplementation("plugin-driver-jvm")
      .setVersion(Utils.lookupVersion(PluginManager::class.java))
      .build()
    return stub.initPlugin(request)
  }

  private fun publishUpdatedCatalogue() {
    val requestBuilder = Plugin.Catalogue.newBuilder()
    CatalogueManager.entries().forEach { (_, entry) ->
      requestBuilder.addCatalogue(Plugin.CatalogueEntry.newBuilder()
        .setKey(entry.key)
        .setType(entry.type.name)
        .putAllValues(entry.values)
        .build())
    }
    val request = requestBuilder.build()

    PLUGIN_REGISTER.forEach { (_, plugin) ->
      plugin.stub?.updateCatalogue(request)
    }
  }

  private fun loadRubyPlugin(manifest: PactPluginManifest): Result<PactPlugin, String> {
    val rvm = lookForProgramInPath("rvm")
    return if (rvm is Ok && manifest.minimumRequiredVersion != null) {
      logger.debug { "Found RVM at ${rvm.value}" }
      startPluginProcess(manifest, mapOf(), rvm.value.toString(), manifest.minimumRequiredVersion.toString(), "do")
    } else {
      when (val ruby = lookForProgramInPath("ruby")) {
        is Ok -> {
          logger.debug { "Found Ruby interpreter at ${ruby.value}" }
          val versionCheck = checkRubyVersion(manifest, ruby)
          if (versionCheck is Err) {
            Err(versionCheck.error)
          } else {
            //            val parent = ruby.value.parent
            //          when (val bundler = lookForProgramInPath("bundle")) {
            //            is Ok -> startPluginProcess(manifest,
            //              mapOf("BUNDLE_GEMFILE" to manifest.pluginDir.resolve("Gemfile").toString()),
            //              bundler.value.toString(), "exec", ruby.value.toString(), "-C${manifest.pluginDir}")
            //            is Err -> {
            //              logger.debug { "Bundler not found in path - ${bundler.error}" }
            //              val bundlePath = parent.resolve("bundle")
            //              if (bundlePath.toFile().exists()) {
            //                startPluginProcess(manifest,
            //                  mapOf("BUNDLE_GEMFILE" to manifest.pluginDir.resolve("Gemfile").toString()),
            //                  bundlePath.toString(), "exec", ruby.value.toString(), "-C${manifest.pluginDir}")
            //              } else {
            //                startPluginProcess(manifest, mapOf(), ruby.value.toString(), "-C${manifest.pluginDir}")
            //              }
            //            }
            //          }
            startPluginProcess(manifest,
              mapOf("BUNDLE_GEMFILE" to manifest.pluginDir.resolve("Gemfile").toString()),
              ruby.value.toString(), "-C${manifest.pluginDir}")
          }
        }
        is Err -> Err(ruby.error)
      }
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
      ProcessBuilder(manifest.pluginDir.resolve(manifest.entryPoint).toString())
    }.directory(manifest.pluginDir)

    val logLevel = logLevel()
    pb.environment()["LOG_LEVEL"] = logLevel
    pb.environment()["RUST_LOG"] = logLevel
    env.forEach { (k, v) -> pb.environment()[k] = v }

    val cp = ChildProcess(pb, manifest)
    return try {
      logger.debug { "Starting plugin ${manifest.name} process ${pb.command()}" }
      cp.start()
      logger.debug { "Plugin ${manifest.name} started with PID ${cp.pid}" }
      val startupInfo = cp.channel.poll(2000, TimeUnit.MILLISECONDS)
      if (startupInfo is JsonObject) {
        Ok(DefaultPactPlugin(cp, manifest, toInteger(startupInfo["port"]), toString(startupInfo["serverKey"])!!))
      } else {
        cp.destroy()
        Err("Plugin process did not output the correct startup message - got $startupInfo")
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

  private fun checkRubyVersion(manifest: PactPluginManifest, ruby: Ok<Path>) =
    if (manifest.minimumRequiredVersion != null) {
      logger.debug { "Checking if Ruby version meets minimum version of ${manifest.minimumRequiredVersion}" }
      when (val rubyOut = SystemExec.execute(ruby.value.toString(), "--version")) {
        is Ok -> {
          logger.debug { "Got Ruby version: ${rubyOut.value}" }
          val rubyVersionStr = rubyOut.value.split(Regex("\\s+"))
          if (rubyVersionStr.size > 1) {
            val rubyVersion = Semver(rubyVersionStr[1].replace(Regex("(p\\d+)"), "+$1"), Semver.SemverType.NPM)
            if (rubyVersion.isLowerThan(manifest.minimumRequiredVersion)) {
              Err("Ruby version $rubyVersion does not meet the minimum version of ${manifest.minimumRequiredVersion}")
            } else {
              Ok("")
            }
          } else {
            Err("Unrecognised ruby version format: ${rubyOut.value}")
          }
        }
        is Err -> Err("Could not execute Ruby interpreter - ${rubyOut.error}")
      }
    } else {
      Ok("")
    }

  private fun loadPluginManifest(name: String, version: String?): Result<PactPluginManifest, String> {
    val manifest = lookupPluginManifest(name, version)
    return if (manifest != null) {
      Ok(manifest)
    } else {
      val pluginDir = System.getenv("PACT_PLUGIN_DIR") ?: System.getenv("HOME") + "/.pact/plugins"
      for (file in File(pluginDir).walk()) {
        if (file.isFile && file.name == "pact-plugin.json") {
          logger.debug { "Found plugin manifest: $file" }
          val pluginJson = file.bufferedReader().use { Json.createReader(it).readObject() }
          if (pluginJson != null) {
            val plugin = DefaultPactPluginManifest.fromJson(file.parentFile, pluginJson)
            if (plugin.name == name && version == null || plugin.version == version) {
              PLUGIN_MANIFEST_REGISTER["$name/${plugin.version}"] = plugin
              return Ok(plugin)
            }
          }
        }
      }
      Err("No plugin with name '$name' and version '${version ?: "any"}' was found in the Pact plugin directory '$pluginDir'")
    }
  }

  private fun lookForProgramInPath(desiredProgram: String): Result<Path, String> {
    val pb = ProcessBuilder(if (SystemUtils.IS_OS_WINDOWS) "where" else "which", desiredProgram)
    return try {
      val proc = pb.start()
      val errCode = proc.waitFor()
      if (errCode == 0) {
        BufferedReader(InputStreamReader(proc.inputStream)).use { reader ->
          Ok(Paths.get(reader.readLine()))
        }
      } else {
        Err("$desiredProgram not found in in PATH")
      }
    } catch (ex: IOException) {
      logger.error(ex) { "Something went wrong while searching for $desiredProgram - ${ex.message}" }
      Err("Something went wrong while searching for $desiredProgram - ${ex.message}")
    } catch (ex: InterruptedException) {
      logger.error(ex) { "Something went wrong while searching for $desiredProgram - ${ex.message}" }
      Err("Something went wrong while searching for $desiredProgram - ${ex.message}")
    }
  }
}

data class PluginConfiguration(
  val interactionConfiguration: MutableMap<String, JsonValue> = mutableMapOf(),
  val pactConfiguration: MutableMap<String, JsonValue> = mutableMapOf()
)

data class InteractionContents @JvmOverloads constructor(
  val body: OptionalBody,
  val rules: MatchingRuleCategory? = null,
  val generators: Generators? = null,
  val metadata: Map<String, JsonValue> = emptyMap(),
  val pluginConfig: PluginConfiguration = PluginConfiguration()
)
