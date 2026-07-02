package io.pact.plugins.jvm.core

import io.github.oshai.kotlinlogging.KotlinLogging
import io.pact.plugin.Plugin
import io.pact.plugins.jvm.core.lua.LuaEngine
import io.pact.plugins.jvm.core.lua.LuaJavaEngine
import org.bouncycastle.asn1.pkcs.RSAPublicKey as Pkcs1RsaPublicKey
import org.bouncycastle.asn1.x509.SubjectPublicKeyInfo
import org.bouncycastle.openssl.PEMKeyPair
import org.bouncycastle.openssl.PEMParser
import org.bouncycastle.openssl.jcajce.JcaPEMKeyConverter
import org.bouncycastle.util.io.pem.PemObject
import org.bouncycastle.util.io.pem.PemWriter
import java.io.File
import java.io.StringReader
import java.io.StringWriter
import java.nio.ByteBuffer
import java.security.KeyFactory
import java.security.PrivateKey
import java.security.PublicKey
import java.security.Signature
import java.security.spec.RSAPublicKeySpec
import java.util.Base64
import java.util.UUID

private val logger = KotlinLogging.logger {}

/**
 * A running Lua plugin instance. Each instance owns its own embedded Lua VM (via [LuaEngine]),
 * running in-process instead of as a separate gRPC child process (see [GrpcPactPlugin]).
 *
 * The plugin script (`entryPoint` in the manifest) must define these global functions:
 * - `init(implementation, version) -> table` - catalogue entries, e.g.
 *   `{ entryType = "CONTENT_MATCHER", key = "...", values = { ... } }`.
 * - `configure_interaction(content_type, config) -> table`
 * - `match_contents(request) -> table`
 * - `generate_content(contents, generators, test_mode)` (optional; passthrough default)
 * - `update_catalogue(catalogue)` (optional; no-op default)
 *
 * A Lua plugin that registers a `TRANSPORT` catalogue entry (instead of, or as well as, a
 * `CONTENT_MATCHER`/`CONTENT_GENERATOR` one) must also define these functions. The plugin
 * itself is responsible for whatever the transport actually requires (opening sockets, making
 * outbound calls, etc.) - this driver only calls these functions at the right points in the
 * test lifecycle, exactly as it would over gRPC for an `exec` plugin (see [LuaPluginRpcClient]):
 * - `start_mock_server(request) -> table`
 * - `shutdown_mock_server(server_key) -> table`
 * - `get_mock_server_results(server_key) -> table`
 * - `prepare_interaction_for_verification(request) -> table`
 * - `verify_interaction(request) -> table`
 */
class LuaPactPlugin(
  override val manifest: PactPluginManifest,
  override val instanceId: String = UUID.randomUUID().toString()
) : PactPlugin {
  override val port: Int? = null
  override val serverKey: String? = null
  override val processPid: Long? = null
  override var catalogueEntries: List<Plugin.CatalogueEntry>? = null
  override var pluginCapabilities: List<String> = emptyList()

  private val engine: LuaEngine = LuaJavaEngine()

  /**
   * Captures this plugin's diagnostic output (`print` and `logger()` calls) into the same
   * per-instance log file a gRPC plugin's stderr is captured to (see
   * [ChildProcess.openLogFile]) - so operators don't need to know which kind of plugin
   * they're looking at to find its log. A Lua plugin runs embedded in the driver's own
   * process, so without this its `print` output would otherwise go straight to the driver's
   * own real stdout, mixed in with everything else.
   */
  private val pluginLog = ChildProcess.openLogFile(manifest, instanceId)

  init {
    engine.addPackagePath(manifest.pluginDir)
    addLuaRocksPath()
    registerHostFunctions()
    engine.loadScript(resolveEntryPoint())
  }

  private fun resolveEntryPoint(): File {
    val entryPoint = File(manifest.entryPoint)
    val path = if (entryPoint.isAbsolute && entryPoint.exists()) {
      entryPoint
    } else {
      File(manifest.pluginDir, manifest.entryPoint)
    }
    require(path.exists()) { "Lua plugin entry point $path does not exist" }
    return path
  }

  /**
   * Makes pure-Lua packages installed via `luarocks` available to `require`, so a plugin can
   * depend on rocks instead of vendoring every third-party library it uses.
   *
   * LuaRocks installs modules under `<rocksDir>/share/lua/<version>/`, where `<rocksDir>`
   * defaults to `~/.luarocks` (its standard per-user tree) but can be a system tree or a
   * custom prefix if the user configured LuaRocks differently. A plugin can override the
   * directory this driver looks in via a `luaRocksDir` key in the manifest's `pluginConfig`.
   * Only the `share/lua` (pure Lua) path is added - packages with compiled C extensions
   * (under `lib/lua`) are not supported. Mirrors the Rust driver's `lua_plugin::add_luarocks_path`.
   */
  private fun addLuaRocksPath() {
    val configured = manifest.pluginConfig["luaRocksDir"] as? String
    val rocksDir = if (configured != null) {
      File(configured)
    } else {
      File(System.getProperty("user.home"), ".luarocks")
    }

    val luaDir = File(rocksDir, "share/lua/$LUAROCKS_LUA_VERSION")
    if (!luaDir.exists()) {
      if (configured != null) {
        logger.debug {
          "Configured luaRocksDir '$rocksDir' does not have a share/lua/$LUAROCKS_LUA_VERSION " +
            "directory, ignoring"
        }
      }
      return
    }

    engine.addPackagePath(luaDir, includeDirectoryModules = true)
    logger.debug { "Added LuaRocks path $luaDir for plugin ${manifest.name}" }
  }

  companion object {
    /** The Lua version this driver embeds - also the version segment LuaRocks uses in its
     * per-version tree layout (e.g. `share/lua/5.4/`). */
    private const val LUAROCKS_LUA_VERSION = "5.4"
  }

  private fun registerHostFunctions() {
    engine.registerFunction("logger") { args ->
      val message = args.getOrNull(0)?.toString()
      logger.debug { "[${manifest.name}] $message" }
      pluginLog?.println(message)
      null
    }
    // Redirects Lua's built-in `print` (its "stdout") into the same per-instance log file, so
    // it doesn't leak into the driver's own real stdout.
    engine.registerFunction("print") { args ->
      pluginLog?.println(args.joinToString("\t") { stringifyForPrint(it) })
      null
    }
    engine.registerFunction("rsa_sign") { args ->
      rsaSign(argToString(args[0]), args[1] as String)
    }
    engine.registerFunction("rsa_public_key") { args ->
      rsaPublicKeyPem(args[0] as String)
    }
    engine.registerFunction("rsa_validate") { args ->
      @Suppress("UNCHECKED_CAST")
      val parts = (args[0] as List<Any?>).map { it as String }
      rsaValidate(parts, args[1] as String, args[2] as String)
    }
    engine.registerFunction("b64_decode_no_pad") { args ->
      ByteBuffer.wrap(decodeBase64Lenient(args[0] as String))
    }
  }

  override fun shutdown() {
    engine.close()
    pluginLog?.close()
  }

  override fun <T> withRpcClient(callback: java.util.function.Function<PactPluginRpcClient, T>): T {
    return callback.apply(LuaPluginRpcClient(engine))
  }
}

/**
 * Formats a value the same way Lua's `tostring()` would, for the `print` host function
 * override. Values here have already been deep-converted from Lua to plain Kotlin types
 * (see `LuaJavaEngine.normalize`), so a whole-numbered `Double` (Lua doesn't distinguish
 * integers from floats at this boundary) is reformatted without the trailing `.0` to match
 * what a real Lua `print` call would have shown.
 */
private fun stringifyForPrint(value: Any?): String = when (value) {
  null -> "nil"
  is Double -> if (value == Math.floor(value) && !value.isInfinite()) value.toLong().toString() else value.toString()
  else -> value.toString()
}

private fun argToString(value: Any?): String = when (value) {
  is ByteBuffer -> {
    val duplicate = value.duplicate()
    val bytes = ByteArray(duplicate.remaining())
    duplicate.get(bytes)
    String(bytes, Charsets.UTF_8)
  }
  else -> value.toString()
}

private fun decodeBase64Lenient(data: String): ByteArray {
  return try {
    Base64.getUrlDecoder().decode(data)
  } catch (e: IllegalArgumentException) {
    val padded = when (data.length % 4) {
      2 -> "$data=="
      3 -> "$data="
      else -> data
    }
    Base64.getUrlDecoder().decode(padded)
  }
}

private val pemConverter = JcaPEMKeyConverter()

private fun parsePkcs1PrivateKey(pem: String): PrivateKey {
  val obj = PEMParser(StringReader(pem)).readObject()
    ?: throw IllegalArgumentException("Could not parse RSA private key PEM")
  val keyPair = obj as? PEMKeyPair
    ?: throw IllegalArgumentException("Expected a PKCS#1 RSA private key PEM, got ${obj.javaClass}")
  return pemConverter.getKeyPair(keyPair).private
}

private fun parsePkcs1PublicKey(pem: String): PublicKey {
  val obj = PEMParser(StringReader(pem)).readObject()
    ?: throw IllegalArgumentException("Could not parse RSA public key PEM")
  val keyInfo = obj as? SubjectPublicKeyInfo
    ?: throw IllegalArgumentException("Expected a PKCS#1 RSA public key PEM, got ${obj.javaClass}")
  return pemConverter.getPublicKey(keyInfo)
}

private fun rsaSign(data: String, privateKeyPem: String): String {
  val privateKey = parsePkcs1PrivateKey(privateKeyPem)
  val signature = Signature.getInstance("SHA512withRSA")
  signature.initSign(privateKey)
  signature.update(data.toByteArray(Charsets.UTF_8))
  return Base64.getUrlEncoder().withoutPadding().encodeToString(signature.sign())
}

private fun rsaPublicKeyPem(privateKeyPem: String): String {
  val privateKey = parsePkcs1PrivateKey(privateKeyPem)
  val publicKeySpec = KeyFactory.getInstance("RSA").let { factory ->
    val privateSpec = factory.getKeySpec(privateKey, java.security.spec.RSAPrivateCrtKeySpec::class.java)
    RSAPublicKeySpec(privateSpec.modulus, privateSpec.publicExponent)
  }
  val pkcs1PublicKey = Pkcs1RsaPublicKey(publicKeySpec.modulus, publicKeySpec.publicExponent)
  val writer = StringWriter()
  PemWriter(writer).use { it.writeObject(PemObject("RSA PUBLIC KEY", pkcs1PublicKey.encoded)) }
  return writer.toString()
}

private fun rsaValidate(tokenParts: List<String>, algorithm: String, publicKeyPem: String): Boolean {
  if (algorithm != "RS512") {
    logger.debug { "Unsupported JWT algorithm '$algorithm': only RS512 is supported" }
    return false
  }
  if (tokenParts.size != 3) {
    logger.debug { "Expected a 3 part JWT token (header, payload, signature)" }
    return false
  }
  return try {
    val publicKey = parsePkcs1PublicKey(publicKeyPem)
    val signatureBytes = decodeBase64Lenient(tokenParts[2])
    val baseToken = "${tokenParts[0]}.${tokenParts[1]}"
    val signature = Signature.getInstance("SHA512withRSA")
    signature.initVerify(publicKey)
    signature.update(baseToken.toByteArray(Charsets.UTF_8))
    signature.verify(signatureBytes)
  } catch (e: Exception) {
    logger.debug(e) { "Failed to validate JWT signature" }
    false
  }
}
