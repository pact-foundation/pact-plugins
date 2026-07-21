package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import io.pact.plugin.v2.PluginV2
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test
import java.io.File

private const val PRIVATE_KEY = """-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAvenHRTv98Lg6FAGkCy35yhcpL+aVw6mYeipYowLGl3zyBfRt
XRlnKAUYPozrfB+/QLqZ+TQMSVamD0q3nYiCwla93IMWscO4MsaVMmljxfl84x7A
djms4hS7IMA9aXOlir0mCPPXE6R89d+pjmErWba+svY/wrl2WCTuqfRLa8bKcgmR
0++35AhzyY8Wxp+8JPBUVSvOH2do1NI5e9tUwFGBUtowKnwT32oC9iYAgo9PcTiv
28mV/FHbqQwRMKbhh0A4SUv2e01YtIuNhvpd3Z74nsdG4lw8VWGyFNbMTrasc1PG
iGReNYTp66S8s+bmivINloxsPrwrstZPE2UJOQIDAQABAoIBAATczXtaU+Ar92C3
wgl/PdwMx8MwNjlySDMojmhuE8OhMVkxrvMpSVje+IXxeb4N2gnAPV0CFiZyj4Ho
udbQvfhX3DifKp+WkUrLhtpplGJnRulRyj+8rk6DlV77TRc8HMr2mNi11ZXtKj3p
YiABIOkFItDWOT+1G/CZ0XqMhLnXq8sfV6Y77eV5ue9G/SeUQlKoW7MA0zth+hBo
ISRo1I8DrJFhJhWhO4OhMTBcV2HbEbbJ9GuD1FA44NJsZPf3DZoq/N0hj9/uopm4
dKVx6Dcr0AP8JN5jjq4CE4hdnz/nr889liwG1C6mElgfsU7Gw6gqKV2PNeO6n+NU
qtKSUnkCgYEA+Ss1DkAL1Rb/z9Ap6VpIpjL84fC/K5HsEjg3rEuEu1xto22MAMz7
rCDelxYXU/NYCeh6sCIQblFYc9hkmmzyJbrcq/yLDZ5HmSOs/RNV+hOTFFFi95VV
5X6OPIjFHzLgo3BjbYtEA+gtoEIMZ/XctfHvcPUssfr2aq6rc5r42+sCgYEAwx6v
eeDYk48mof2GrOD8yJvNQHL9iJXXQ/DJ6it14R5JO2iNbX3y9TDb2Xu+KQU6/66g
095M3JlmeyT8/eFMwH5978Ci2pmDEs+QZXG6GwFFEwRxTMdQoHDMgue8TMLm3FJd
D9FXPk9wKBGjGN3DB5G3AzHqVqaN+Xij9/aR2msCgYBSlwLIDWyenjf+zxYFVjq8
dCwkTCNhssWYKHAzuPhvDiz9PcNpRIirPl3poJXs6r0k051PIotltarnAzQdh70f
ynd4voXs5qj+1rdxT2ZxNOnMk0mFnUdSgYduAzuroraZFhiu57mMvfnZo+ruzqzw
1heyzmGZQQFKzUjhUd3pLwKBgGCKTDQ3ZbEMwQahVAMxhqETRWi//GWaDdpVxvGP
81EhFQbJ4j/sc0uRkxV2Pk45gkmDc5ugf9MeKzB+ypYq5TjQ3SrE207haZLjFAS9
UmGOLUkNh6l/bIsVhHq4gdhRDrywG895unrf/xQ0NchV4Otb03tHNTUOT2zBng9P
9jZlAoGBAIolo+I7P3pMo87uy5qDDmxQaCj9wsIzKbliTpDb3WvmHimpaCCGOgbi
Oz4QOdgkf+Unl1cOnF8EAQ0J2bp+Cck7kb8u3cjKY1AR17ugIksOaB9mGB0bJ7hu
tnS+LGbydGz22ZMCG6LF0Z+dNX0zZoWKsvGAWTJBVSANnTo95igh
-----END RSA PRIVATE KEY-----"""

class LuaPactPluginTest {
  private fun jwtManifest(): PactPluginManifest {
    val pluginDir = File("../../../plugins/jwt").canonicalFile
    require(pluginDir.exists()) { "plugins/jwt directory should exist at $pluginDir" }
    return DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = "jwt",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "plugin.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList()
    )
  }

  @Test
  fun `loads the jwt plugin and runs the init function`() {
    val plugin = LuaPactPlugin(jwtManifest())
    try {
      val response = plugin.withRpcClient {
        it.initPlugin(PluginInitRequest(implementation = "test", version = "0.0.0"))
      }
      assertEquals(2, response.catalogueEntries.size)
      assertEquals("jwt", response.catalogueEntries[0].key)
      assertEquals(Plugin.CatalogueEntry.EntryType.CONTENT_MATCHER, response.catalogueEntries[0].type)
      assertEquals(Plugin.CatalogueEntry.EntryType.CONTENT_GENERATOR, response.catalogueEntries[1].type)
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `configure_interaction then compareContents round trip`() {
    val plugin = LuaPactPlugin(jwtManifest())
    try {
      val config = Utils.mapToProtoStruct(
        mapOf(
          "private-key" to PRIVATE_KEY,
          "subject" to "test-subject",
          "issuer" to "test-issuer",
          "audience" to "test-audience",
          "algorithm" to "RS512"
        )
      )
      val configureRequest = Plugin.ConfigureInteractionRequest.newBuilder()
        .setContentType("application/jwt+json")
        .setContentsConfig(config)
        .build()
      val configureResponse = plugin.withRpcClient { it.configureInteraction(configureRequest) }
      assertEquals("", configureResponse.error)
      assertEquals(1, configureResponse.interactionCount)

      val interaction = configureResponse.getInteraction(0)
      assertTrue(interaction.hasContents())
      assertEquals("application/jwt+json", interaction.contents.contentType)
      val token = interaction.contents.content.value.toStringUtf8()
      assertEquals(3, token.split(".").size)

      val compareRequest = Plugin.CompareContentsRequest.newBuilder()
        .setExpected(interaction.contents)
        .setActual(interaction.contents)
        .setAllowUnexpectedKeys(false)
        .setPluginConfiguration(interaction.pluginConfiguration)
        .build()
      val compareResponse = plugin.withRpcClient { it.compareContents(compareRequest) }
      assertEquals("", compareResponse.error)
      assertFalse(compareResponse.hasTypeMismatch())
      assertTrue(compareResponse.resultsMap.isEmpty(), "expected no mismatches, got ${compareResponse.resultsMap}")
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `compareContents detects a tampered token`() {
    val plugin = LuaPactPlugin(jwtManifest())
    try {
      val config = Utils.mapToProtoStruct(
        mapOf("private-key" to PRIVATE_KEY, "algorithm" to "RS512")
      )
      val configureRequest = Plugin.ConfigureInteractionRequest.newBuilder()
        .setContentType("application/jwt+json")
        .setContentsConfig(config)
        .build()
      val configureResponse = plugin.withRpcClient { it.configureInteraction(configureRequest) }
      val interaction = configureResponse.getInteraction(0)
      val expectedBody = interaction.contents
      val tamperedToken = expectedBody.content.value.toStringUtf8() + "x"
      val actualBody = expectedBody.toBuilder()
        .setContent(com.google.protobuf.BytesValue.of(com.google.protobuf.ByteString.copyFromUtf8(tamperedToken)))
        .build()

      val compareRequest = Plugin.CompareContentsRequest.newBuilder()
        .setExpected(expectedBody)
        .setActual(actualBody)
        .setAllowUnexpectedKeys(false)
        .setPluginConfiguration(interaction.pluginConfiguration)
        .build()
      val compareResponse = plugin.withRpcClient { it.compareContents(compareRequest) }
      assertTrue(compareResponse.resultsMap.isNotEmpty(), "expected a mismatch to be detected")
    } finally {
      plugin.shutdown()
    }
  }

  private fun hostCallbackManifest(pluginDir: File, name: String, script: String): PactPluginManifest {
    File(pluginDir, "entry.lua").writeText(script)
    return DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = name,
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList()
    )
  }

  @Test
  fun `match_contents calls host_compare_contents for a registered core capability`() {
    val key = "match_contents-calls-host_compare_contents-for-a-registered-core-capability"
    CatalogueManager.registerCoreEntries(listOf(
      CatalogueEntry(CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.CORE, "", key)
    ))
    CoreCapabilityRegistry.registerContentMatcher(key) {
      Plugin.CompareContentsResponse.newBuilder().setError("core matcher says no").build()
    }

    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-compare-contents-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-compare-contents-test",
      """
        function match_contents(request)
          return host_compare_contents("$key", request)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient { it.compareContents(Plugin.CompareContentsRequest.newBuilder().build()) }
      assertEquals("core matcher says no", response.error)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
      CoreCapabilityRegistry.deregisterContentMatcher(key)
    }
  }

  @Test
  fun `match_contents surfaces a clear error when host_compare_contents targets an unregistered entry`() {
    val key = "match_contents-surfaces-a-clear-error-when-host_compare_contents-targets-an-unregistered-entry"
    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-compare-contents-missing-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-compare-contents-missing-test",
      """
        function match_contents(request)
          return host_compare_contents("$key", request)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val ex = assertThrows(RuntimeException::class.java) {
        plugin.withRpcClient { it.compareContents(Plugin.CompareContentsRequest.newBuilder().build()) }
      }
      assertTrue(
        ex.message?.contains("No catalogue entry found") == true,
        "expected a 'No catalogue entry found' error, got: ${ex.message}"
      )
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `generate_content calls host_generate_content for a registered core capability`() {
    val key = "generate_content-calls-host_generate_content-for-a-registered-core-capability"
    CatalogueManager.registerCoreEntries(listOf(
      CatalogueEntry(CatalogueEntryType.CONTENT_GENERATOR, CatalogueEntryProviderType.CORE, "", key)
    ))
    CoreCapabilityRegistry.registerContentGenerator(key) {
      Plugin.GenerateContentResponse.newBuilder()
        .setContents(
          Plugin.Body.newBuilder()
            .setContentType("text/plain")
            .setContent(com.google.protobuf.BytesValue.of(com.google.protobuf.ByteString.copyFromUtf8("generated by the host")))
            .build()
        )
        .build()
    }

    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-generate-content-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-generate-content-test",
      """
        function generate_content(contents, generators, test_mode)
          return host_generate_content("$key", contents, generators, test_mode)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val request = Plugin.GenerateContentRequest.newBuilder()
        .setContents(
          Plugin.Body.newBuilder()
            .setContentType("text/plain")
            .setContent(com.google.protobuf.BytesValue.of(com.google.protobuf.ByteString.copyFromUtf8("original")))
            .build()
        )
        .build()
      val response = plugin.withRpcClient { it.generateContent(request) }
      assertEquals("generated by the host", response.contents.content.value.toStringUtf8())
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
      CoreCapabilityRegistry.deregisterContentGenerator(key)
    }
  }

  @Test
  fun `captures print and logger output into the per-instance log file`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-plugin-log-test").toFile()
    File(pluginDir, "entry.lua").writeText(
      """
        print("hello", "world", 42)
        logger("a logger message")
      """.trimIndent()
    )
    val instanceId = "test-instance-${System.nanoTime()}"
    val manifest = DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = "log-test",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList()
    )

    val plugin = LuaPactPlugin(manifest, instanceId)
    plugin.shutdown()

    val logFile = File(ChildProcess.pluginLogDir(), "pact-plugin-log-test-$instanceId.log")
    try {
      assertTrue(logFile.exists(), "Expected a log file at $logFile")
      val nl = System.lineSeparator()
      assertEquals("hello\tworld\t42$nl" + "a logger message$nl", logFile.readText())
    } finally {
      logFile.delete()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `loads pure Lua packages from a configured luaRocksDir`() {
    val rocksRoot = kotlin.io.path.createTempDirectory("luarocks-test").toFile()
    val luaDir = File(rocksRoot, "share/lua/5.4")
    luaDir.mkdirs()
    File(luaDir, "greeter.lua").writeText(
      """return { hello = function() return "hello from luarocks" end }"""
    )

    val pluginDir = kotlin.io.path.createTempDirectory("lua-plugin-test").toFile()
    File(pluginDir, "entry.lua").writeText(
      """
        local greeter = require "greeter"
        function init(implementation, version)
          return { { entryType = "CONTENT_MATCHER", key = greeter.hello(), values = {} } }
        end
      """.trimIndent()
    )

    val manifest = DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = "luarocks-test",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList(),
      pluginConfig = mapOf("luaRocksDir" to rocksRoot.absolutePath)
    )

    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient {
        it.initPlugin(PluginInitRequest(implementation = "test", version = "0.0.0"))
      }
      assertEquals("hello from luarocks", response.catalogueEntries[0].key)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
      rocksRoot.deleteRecursively()
    }
  }

  @Test
  fun `loads a vendored directory-style module from the plugin directory`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-plugin-test").toFile()
    val moduleDir = File(pluginDir, "greeter")
    moduleDir.mkdirs()
    File(moduleDir, "init.lua").writeText(
      """return { hello = function() return "hello from a vendored module" end }"""
    )
    File(pluginDir, "entry.lua").writeText(
      """
        local greeter = require "greeter"
        function init(implementation, version)
          return { { entryType = "CONTENT_MATCHER", key = greeter.hello(), values = {} } }
        end
      """.trimIndent()
    )

    val manifest = DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = "vendored-module-test",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList(),
      pluginConfig = emptyMap()
    )

    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient {
        it.initPlugin(PluginInitRequest(implementation = "test", version = "0.0.0"))
      }
      assertEquals("hello from a vendored module", response.catalogueEntries[0].key)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `ignores a missing luaRocksDir instead of failing`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-plugin-test").toFile()
    File(pluginDir, "entry.lua").writeText("-- no-op")

    val manifest = DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 1,
      name = "luarocks-test",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList(),
      pluginConfig = mapOf("luaRocksDir" to "/no/such/directory")
    )

    val plugin = LuaPactPlugin(manifest)
    plugin.shutdown()
    pluginDir.deleteRecursively()
  }

  private val transportPluginScript = """
    function start_mock_server(request)
      START_MOCK_SERVER_REQUEST = request
      if request.port == 0 then
        return { error = "could not bind a mock server" }
      end
      return { details = { key = "mock-server-1", port = 12345, address = "127.0.0.1:12345" } }
    end

    function shutdown_mock_server(server_key)
      SHUTDOWN_SERVER_KEY = server_key
      return {
        ok = false,
        results = { { path = "/foo", error = "did not match", mismatches = { "simple string mismatch" } } }
      }
    end

    function get_mock_server_results(server_key)
      GET_RESULTS_SERVER_KEY = server_key
      return { ok = true, results = {} }
    end

    function prepare_interaction_for_verification(request)
      PREPARE_REQUEST = request
      return {
        interaction_data = {
          body = { content_type = "application/json", contents = "prepared-body", content_type_hint = "TEXT" },
          metadata = { path = "/foo", tag = { binary = "raw-bytes" } }
        }
      }
    end

    function verify_interaction(request)
      VERIFY_REQUEST = request
      return {
        result = {
          success = true,
          response_data = { body = { content_type = "application/json", contents = "response-body" }, metadata = {} },
          mismatches = { "a plain mismatch", { mismatch = "a table mismatch", path = "${'$'}.foo", expected = 1, actual = 2 } },
          output = { "POST /foo", "200 OK" }
        }
      }
    end
  """.trimIndent()

  private fun transportManifest(pluginDir: File, pluginInterfaceVersion: Int): PactPluginManifest {
    File(pluginDir, "entry.lua").writeText(transportPluginScript)
    return DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = pluginInterfaceVersion,
      name = "transport-test",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "entry.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList()
    )
  }

  @Test
  fun `startMockServer v1 round trip`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 1))
    try {
      val request = Plugin.StartMockServerRequest.newBuilder()
        .setHostInterface("127.0.0.1")
        .setPort(8080)
        .setPact("{\"consumer\":{}}")
        .build()
      val response = plugin.withRpcClient { it.startMockServer(request) }
      assertTrue(response.hasDetails())
      assertEquals("mock-server-1", response.details.key)
      assertEquals(12345, response.details.port)
      assertEquals("127.0.0.1:12345", response.details.address)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `startMockServer v1 returns the lua error`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 1))
    try {
      val request = Plugin.StartMockServerRequest.newBuilder()
        .setHostInterface("127.0.0.1")
        .setPort(0)
        .setPact("{}")
        .build()
      val response = plugin.withRpcClient { it.startMockServer(request) }
      assertTrue(response.hasError())
      assertEquals("could not bind a mock server", response.error)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `startMockServer v2 passes structured interactions`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 2))
    try {
      val request = PluginV2.StartMockServerRequest.newBuilder()
        .setHostInterface("127.0.0.1")
        .setPort(8080)
        .addInteractions(
          PluginV2.InteractionContents.newBuilder()
            .setInteractionType("Synchronous/HTTP")
            .setConsumer("test-consumer")
            .setProvider("test-provider")
            .build()
        )
        .build()
      val response = plugin.withRpcClient { it.startMockServerV2(request) }
      assertTrue(response.hasDetails())
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `shutdown and get mock server results parse mismatches`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 1))
    try {
      val shutdownResponse = plugin.withRpcClient {
        it.shutdownMockServer(Plugin.ShutdownMockServerRequest.newBuilder().setServerKey("mock-server-1").build())
      }
      assertFalse(shutdownResponse.ok)
      assertEquals(1, shutdownResponse.resultsCount)
      assertEquals("/foo", shutdownResponse.getResults(0).path)
      assertEquals("simple string mismatch", shutdownResponse.getResults(0).getMismatches(0).mismatch)

      val resultsResponse = plugin.withRpcClient {
        it.getMockServerResults(Plugin.MockServerRequest.newBuilder().setServerKey("mock-server-1").build())
      }
      assertTrue(resultsResponse.ok)
      assertEquals(0, resultsResponse.resultsCount)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `prepareInteractionForVerification v1 round trip`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 1))
    try {
      val request = Plugin.VerificationPreparationRequest.newBuilder()
        .setPact("{}")
        .setInteractionKey("interaction-1")
        .build()
      val response = plugin.withRpcClient { it.prepareInteractionForVerification(request) }
      assertTrue(response.hasInteractionData())
      assertEquals("prepared-body", response.interactionData.body.content.value.toStringUtf8())
      val metadata = response.interactionData.metadataMap
      assertTrue(metadata["path"]!!.hasNonBinaryValue())
      assertTrue(metadata["tag"]!!.hasBinaryValue())
      assertEquals("raw-bytes", metadata["tag"]!!.binaryValue.toStringUtf8())
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `prepareInteractionForVerification v2 passes interaction contents`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 2))
    try {
      val request = PluginV2.VerificationPreparationRequest.newBuilder()
        .setInteractionContents(
          PluginV2.InteractionContents.newBuilder()
            .setInteractionType("Synchronous/HTTP")
            .setConsumer("test-consumer")
            .setProvider("test-provider")
            .build()
        )
        .build()
      val response = plugin.withRpcClient { it.prepareInteractionForVerificationV2(request) }
      assertTrue(response.hasInteractionData())
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `verifyInteraction v1 round trip`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 1))
    try {
      val interactionData = Plugin.InteractionData.newBuilder()
        .setBody(
          Plugin.Body.newBuilder()
            .setContentType("application/json")
            .setContent(com.google.protobuf.BytesValue.of(com.google.protobuf.ByteString.copyFromUtf8("request-body")))
            .build()
        )
        .putMetadata(
          "path",
          Plugin.MetadataValue.newBuilder()
            .setNonBinaryValue(com.google.protobuf.Value.newBuilder().setStringValue("/foo").build())
            .build()
        )
        .build()
      val request = Plugin.VerifyInteractionRequest.newBuilder()
        .setInteractionData(interactionData)
        .setPact("{}")
        .setInteractionKey("interaction-1")
        .build()
      val response = plugin.withRpcClient { it.verifyInteraction(request) }
      assertTrue(response.hasResult())
      assertTrue(response.result.success)
      assertEquals(listOf("POST /foo", "200 OK"), response.result.outputList)
      assertEquals(2, response.result.mismatchesCount)
      assertTrue(response.result.getMismatches(0).hasError())
      assertEquals("a plain mismatch", response.result.getMismatches(0).error)
      assertTrue(response.result.getMismatches(1).hasMismatch())
      assertEquals("a table mismatch", response.result.getMismatches(1).mismatch.mismatch)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `verifyInteraction v2 converts the v2 interaction data and contents`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-transport-plugin-test").toFile()
    val plugin = LuaPactPlugin(transportManifest(pluginDir, 2))
    try {
      val interactionData = PluginV2.InteractionData.newBuilder()
        .setBody(
          PluginV2.Body.newBuilder()
            .setContentType("application/json")
            .setContent(com.google.protobuf.BytesValue.of(com.google.protobuf.ByteString.copyFromUtf8("request-body")))
            .build()
        )
        .build()
      val request = PluginV2.VerifyInteractionRequest.newBuilder()
        .setInteractionData(interactionData)
        .setInteractionContents(
          PluginV2.InteractionContents.newBuilder()
            .setInteractionType("Synchronous/HTTP")
            .setConsumer("test-consumer")
            .setProvider("test-provider")
            .build()
        )
        .build()
      val response = plugin.withRpcClient { it.verifyInteractionV2(request) }
      assertTrue(response.hasResult())
      assertTrue(response.result.success)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }
}
