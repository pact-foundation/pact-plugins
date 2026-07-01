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
 * Only content-matching (`compareContents`/`configureInteraction`/`generateContent`) is
 * implemented. Mock-server and `verifyInteraction`/`prepareInteractionForVerification` are
 * only ever invoked for `TRANSPORT`-registered plugins, not `CONTENT_MATCHER`/
 * `CONTENT_GENERATOR` ones, so they throw [UnsupportedOperationException] here.
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

  init {
    engine.addPackagePath(manifest.pluginDir)
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

  private fun registerHostFunctions() {
    engine.registerFunction("logger") { args ->
      logger.debug { "[${manifest.name}] ${args.getOrNull(0)}" }
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
  }

  override fun <T> withRpcClient(callback: java.util.function.Function<PactPluginRpcClient, T>): T {
    return callback.apply(LuaPluginRpcClient(engine))
  }
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
