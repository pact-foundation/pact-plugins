package io.pact.plugins.jvm.core

import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.mapError
import com.vdurmont.semver4j.Semver
import io.grpc.ManagedChannel
import io.grpc.ManagedChannelBuilder
import io.pact.core.model.OptionalBody
import io.pact.core.support.Json
import io.pact.core.support.Utils
import io.pact.core.support.handleWith
import io.pact.core.support.json.JsonParser
import io.pact.core.support.json.JsonValue
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.PactPluginGrpc.newBlockingStub
import io.pact.plugin.Plugin
import mu.KLogging
import org.apache.commons.lang3.SystemUtils
import java.io.BufferedReader
import java.io.File
import java.io.IOException
import java.io.InputStreamReader
import java.lang.Runtime.getRuntime
import java.lang.RuntimeException
import java.lang.reflect.InvocationTargetException
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import kotlin.reflect.full.createInstance
import kotlin.reflect.full.declaredMemberFunctions
import kotlin.reflect.full.memberFunctions
import kotlin.reflect.jvm.jvmErasure

interface PactPluginManifest {
  val pluginDir: File
  val pluginInterfaceVersion: Int
  val name: String
  val version: String
  val executableType: String
  val minimumRequiredVersion: String?
  val entryPoint: String
  val dependencies: List<String>
}

data class DefaultPactPluginManifest(
  override val pluginDir: File,
  override val pluginInterfaceVersion: Int,
  override val name: String,
  override val version: String,
  override val executableType: String,
  override val minimumRequiredVersion: String?,
  override val entryPoint: String,
  override val dependencies: List<String>
): PactPluginManifest {
  companion object {
    fun fromJson(pluginDir: File, pluginJson: JsonValue.Object): PactPluginManifest {
      return DefaultPactPluginManifest(
        pluginDir,
        Json.toInteger(pluginJson["pluginInterfaceVersion"]) ?: 1,
        Json.toString(pluginJson["name"]),
        Json.toString(pluginJson["version"]),
        Json.toString(pluginJson["executableType"]),
        Json.toString(pluginJson["minimumRequiredVersion"]),
        Json.toString(pluginJson["entryPoint"]),
        listOf()
      )
    }
  }
}

interface PactPlugin {
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
  fun loadPlugin(name: String): Result<PactPlugin, String>

  /**
   * Invoke the content type matcher
   */
  fun invokeContentMatcher(
    matcher: ContentMatcher,
    expected: OptionalBody,
    actual: OptionalBody,
    context: Any
  ): Any?
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

  override fun loadPlugin(name: String): Result<PactPlugin, String> {
    return if (PLUGIN_REGISTER.containsKey(name)) {
      Ok(PLUGIN_REGISTER[name]!!)
    } else {
      when (val manifest = loadPluginManifest(name)) {
        is Ok -> initialisePlugin(manifest.value)
        is Err -> Err(manifest.error)
      }
    }
  }

  override fun invokeContentMatcher(
    matcher: ContentMatcher,
    expected: OptionalBody,
    actual: OptionalBody,
    context: Any
  ): Any? {
    return when {
      matcher is CatalogueContentMatcher && matcher.isCore -> {
        val clazz = Class.forName(matcher.catalogueEntry.values["implementation"]).kotlin
        val bodyMatcher = clazz.objectInstance ?: clazz.createInstance()
        try {
          clazz.memberFunctions.find { it.name == "matchBody" }!!.call(bodyMatcher, expected, actual, context)
        } catch (e: InvocationTargetException) {
          throw e.targetException
        }
      }
      matcher is CatalogueContentMatcher -> {
        val request = Plugin.CompareContentsRequest.newBuilder()
          .setExpected(Plugin.Body.newBuilder())
          .setActual(Plugin.Body.newBuilder())
          .setContext(com.google.protobuf.Struct.parseFrom(
            Json.toJson(context).serialise().toByteArray()
          ))
          .build()
        PLUGIN_REGISTER[matcher.catalogueEntry.key]!!.stub!!.compareContents(request)
      }
      matcher.isCore -> {
        try {
          matcher::class.memberFunctions.find { it.name == "matchBody" }!!.call(matcher, expected, actual, context)
        } catch (e: InvocationTargetException) {
          throw e.targetException
        }
      }
      else -> throw RuntimeException("Mis-configured content type matcher $matcher")
    }
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
        PLUGIN_REGISTER[manifest.name] = plugin
        logger.debug { "Plugin process started OK (port = ${plugin.port}), sending init message" }
        handleWith<PactPlugin> {
          val request = Plugin.InitPluginRequest.newBuilder()
            .setImplementation("Pact-JVM")
            .setVersion(Utils.lookupVersion(PluginManager::class.java))
            .build()
          val channel = ManagedChannelBuilder.forTarget("127.0.0.1:${plugin.port}")
            .usePlaintext()
            .build()
          val stub = newBlockingStub(channel)
          val response = stub.initPlugin(request)
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

    env.forEach { (k, v) -> pb.environment()[k] = v }

    val cp = ChildProcess(pb, manifest)
    return try {
      logger.debug { "Starting plugin ${manifest.name} process ${pb.command()}" }
      cp.start()
      logger.debug { "Plugin ${manifest.name} started with PID ${cp.pid}" }
      val startupInfo = cp.channel.poll(2000, TimeUnit.MILLISECONDS)
      if (startupInfo is JsonValue.Object) {
        Ok(DefaultPactPlugin(cp, Json.toInteger(startupInfo["port"]), Json.toString(startupInfo["serverKey"])))
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

  private fun loadPluginManifest(name: String): Result<PactPluginManifest, String> {
  return if (PLUGIN_MANIFEST_REGISTER.containsKey(name)) {1z k2qaaq  tv     c1`
      Ok(PLUGIN_MANIFEST_REGISTER[name]!!)
    } else {
      val pluginDir = System.getenv("PACT_PLUGIN_DIR") ?: System.getenv("HOME") + "/.pact/plugins"
      for (file in File(pluginDir).walk()) {
        if (file.isFile && file.name == "pact-plugin.json") {
          logger.debug { "Found plugin manifest: $file" }
          val pluginJson = file.bufferedReader().use { JsonParser.parseReader(it) }
          if (pluginJson.isObject) {
            val plugin = DefaultPactPluginManifest.fromJson(file.parentFile, pluginJson.asObject()!!)
            if (plugin.name == name) {
              PLUGIN_MANIFEST_REGISTER[name] = plugin
              return Ok(plugin)
            }
          }
        }
      }
      Err("No plugin with name '$name' was found in the Pact plugin directory '$pluginDir'")
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
