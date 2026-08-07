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

  /**
   * A plugin can return the interaction's matching rules from `configure_interaction`, so a rule on
   * something inside a content type the framework can not traverse still ends up in the Pact file's
   * `matchingRules` rather than in the plugin's own configuration.
   */
  @Test
  fun `configure_interaction carries matching rules from the plugin`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-configure-rules-test").toFile()
    val manifest = hostCallbackManifest(pluginDir, "configure-rules-test", """
      function configure_interaction(content_type, config)
        return {
          interactions = {
            {
              contents = { contents = "a-body", content_type = content_type },
              rules = {
                ["${'$'}.one"] = { { type = "regex", values = { regex = "\\d+" } } },
                ["${'$'}.two"] = { { type = "type" } }
              }
            }
          }
        }
      end
    """.trimIndent())
    val plugin = LuaPactPlugin(manifest)

    try {
      val response = plugin.withRpcClient {
        it.configureInteraction(Plugin.ConfigureInteractionRequest.newBuilder()
          .setContentType("application/x-test")
          .build())
      }

      val interaction = response.getInteraction(0)
      val regexRule = interaction.rulesMap["${'$'}.one"]!!.getRule(0)
      assertEquals("regex", regexRule.type)
      assertEquals("\\d+", Utils.structToMap(regexRule.values)["regex"])
      assertEquals("type", interaction.rulesMap["${'$'}.two"]!!.getRule(0).type)
    } finally {
      plugin.shutdown()
    }
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

  // ---- Field-level matchers and generators (proposal 006) ----

  private fun creditcardManifest(): PactPluginManifest {
    val pluginDir = File("../../../plugins/creditcard").canonicalFile
    require(pluginDir.exists()) { "plugins/creditcard directory should exist at $pluginDir" }
    return DefaultPactPluginManifest(
      pluginDir = pluginDir,
      pluginInterfaceVersion = 2,
      name = "creditcard",
      version = "0.0.0",
      executableType = "lua",
      minimumRequiredVersion = null,
      entryPoint = "plugin.lua",
      entryPoints = emptyMap(),
      args = emptyList(),
      dependencies = emptyList()
    )
  }

  private fun textField(value: String): PluginV2.FieldValue =
    PluginV2.FieldValue.newBuilder().setStringValue(value).build()

  private fun brandValues(brand: String?): com.google.protobuf.Struct? =
    brand?.let { Utils.mapToProtoStruct(mapOf("brand" to it)) }

  /** A `MatchFieldRequest` for the `creditcard` rule, optionally configured with a brand. */
  private fun creditcardMatchRequest(brand: String?, expected: String, actual: String): PluginV2.MatchFieldRequest {
    val rule = PluginV2.MatchingRule.newBuilder().setType("creditcard")
    brandValues(brand)?.let { rule.values = it }
    return PluginV2.MatchFieldRequest.newBuilder()
      .setKey("creditcard")
      .setRule(rule.build())
      .setPath("$.card.number")
      .setMismatchType("body")
      .setExpected(textField(expected))
      .setActual(textField(actual))
      .build()
  }

  private fun creditcardGenerateRequest(brand: String?, example: String): PluginV2.GenerateFieldRequest {
    val generator = PluginV2.Generator.newBuilder().setType("creditcard")
    brandValues(brand)?.let { generator.values = it }
    return PluginV2.GenerateFieldRequest.newBuilder()
      .setKey("creditcard")
      .setGenerator(generator.build())
      .setPath("$.card.number")
      .setExampleValue(textField(example))
      .setTestMode(PluginV2.GenerateContentRequest.TestMode.Consumer)
      .build()
  }

  @Test
  fun `creditcard plugin registers a matcher and a generator under the same key`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.initPlugin(PluginInitRequest(implementation = "test", version = "0.0.0"))
      }
      assertEquals(2, response.catalogueEntries.size)
      assertEquals("creditcard", response.catalogueEntries[0].key)
      assertEquals(CatalogueEntryType.MATCHER.toEntryValue(), response.catalogueEntries[0].typeValue)
      assertEquals("creditcard", response.catalogueEntries[1].key)
      assertEquals(CatalogueEntryType.GENERATOR.toEntryValue(), response.catalogueEntries[1].typeValue)
      // The values key that maps a single positional config argument in a rule definition
      assertEquals("brand", response.catalogueEntries[0].valuesMap["config-key"])
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin accepts a valid card number`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest("visa", "4111111111111111", "4012888888881881"))
      }
      assertEquals("", response.error)
      assertTrue(
        response.mismatchesList.isEmpty(),
        "expected no mismatches, got ${response.mismatchesList}"
      )
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin reports a number that fails the Luhn check`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest(null, "4111111111111111", "4111111111111112"))
      }
      assertEquals("", response.error)
      assertEquals(1, response.mismatchesCount)
      val mismatch = response.getMismatches(0)
      assertTrue(
        mismatch.mismatch.contains("Luhn check"),
        "unexpected mismatch description: ${mismatch.mismatch}"
      )
      // The plugin places its own mismatches, echoing back the path and part it was given
      assertEquals("$.card.number", mismatch.path)
      assertEquals("body", mismatch.mismatchType)
      assertEquals("4111111111111111", mismatch.expected.value.toStringUtf8())
      assertEquals("4111111111111112", mismatch.actual.value.toStringUtf8())
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin reports a number from the wrong brand`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      // A valid Visa number, but the rule asks for a Mastercard
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest("mastercard", "5555555555554444", "4012888888881881"))
      }
      assertEquals(1, response.mismatchesCount)
      assertTrue(
        response.getMismatches(0).mismatch.contains("Mastercard"),
        "unexpected mismatch description: ${response.getMismatches(0).mismatch}"
      )
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin reports a misconfigured brand as an error not a mismatch`() {
    // The test author's mistake, not the provider's - so it fails the test outright rather than
    // being reported as the provider sending the wrong value.
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest("amx", "4111111111111111", "4111111111111111"))
      }
      assertTrue(
        response.error.contains("'amx' is not a credit card brand"),
        "unexpected error: ${response.error}"
      )
      assertTrue(response.mismatchesList.isEmpty())
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin generates a number for the configured brand`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.generateField(creditcardGenerateRequest("amex", "4111111111111111"))
      }
      assertEquals("", response.error)
      assertEquals(PluginV2.FieldValue.ValueCase.STRINGVALUE, response.value.valueCase)
      val generated = response.value.stringValue
      assertEquals(15, generated.length, "an Amex number has 15 digits, got '$generated'")
      assertTrue(generated.startsWith("34") || generated.startsWith("37"), "got '$generated'")

      // And the number it generated is one it will accept back
      val matchResponse = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest("amex", "371449635398431", generated))
      }
      assertTrue(
        matchResponse.mismatchesList.isEmpty(),
        "the plugin should accept its own generated number, got ${matchResponse.mismatchesList}"
      )
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `creditcard plugin reports a generator error`() {
    val plugin = LuaPactPlugin(creditcardManifest())
    try {
      val response = plugin.withRpcClient {
        it.generateField(creditcardGenerateRequest("amx", "4111111111111111"))
      }
      assertTrue(response.error.contains("amx"), "unexpected error: ${response.error}")
      assertFalse(response.hasValue())
    } finally {
      plugin.shutdown()
    }
  }

  @Test
  fun `each field value type survives the round trip through Lua`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-field-value-round-trip-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "field-value-round-trip-test",
      """
        function generate_field(request)
          return { value = request.example_value }
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val values = listOf(
        PluginV2.FieldValue.newBuilder().setNullValueValue(0).build(),
        PluginV2.FieldValue.newBuilder().setBooleanValue(true).build(),
        PluginV2.FieldValue.newBuilder().setStringValue("4111111111111111").build(),
        PluginV2.FieldValue.newBuilder().setIntegerValue(100).build(),
        PluginV2.FieldValue.newBuilder().setDecimalValue(100.5).build(),
        // Deliberately starts with a NUL and is not valid UTF-8: a Lua string is an arbitrary
        // byte array, and reading one back as a Java String would truncate this to nothing
        PluginV2.FieldValue.newBuilder()
          .setBinaryValue(com.google.protobuf.ByteString.copyFrom(byteArrayOf(0, -97, -110, -106)))
          .build()
      )
      for (value in values) {
        val request = PluginV2.GenerateFieldRequest.newBuilder().setExampleValue(value).build()
        val response = plugin.withRpcClient { it.generateField(request) }
        assertEquals(value, response.value, "$value did not survive the round trip through Lua")
      }
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `a whole number reaches Lua as an integer and a decimal as a float`() {
    // The distinction the integer, decimal and type rules are built on. Lua 5.4 has separate
    // integer and float subtypes, so it can be checked from inside the script itself.
    val pluginDir = kotlin.io.path.createTempDirectory("lua-field-value-type-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "field-value-lua-type-test",
      """
        function generate_field(request)
          return { value = math.type(request.example_value) }
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val luaTypeOf = { value: PluginV2.FieldValue ->
        val request = PluginV2.GenerateFieldRequest.newBuilder().setExampleValue(value).build()
        plugin.withRpcClient { it.generateField(request) }.value.stringValue
      }
      assertEquals("integer", luaTypeOf(PluginV2.FieldValue.newBuilder().setIntegerValue(100).build()))
      assertEquals("float", luaTypeOf(PluginV2.FieldValue.newBuilder().setDecimalValue(100.0).build()))
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `a mismatch that does not place itself is reported against the request path`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-field-mismatch-path-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "field-mismatch-path-test",
      """
        function match_field(request)
          return { mismatches = { "not a card number" } }
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest(null, "4111111111111111", "nope"))
      }
      assertEquals(1, response.mismatchesCount)
      assertEquals("not a card number", response.getMismatches(0).mismatch)
      assertEquals("$.card.number", response.getMismatches(0).path)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `a plugin that does not define the field functions says so`() {
    val pluginDir = kotlin.io.path.createTempDirectory("lua-field-functions-missing-test").toFile()
    val manifest = hostCallbackManifest(pluginDir, "field-functions-missing-test", "-- nothing here")
    val plugin = LuaPactPlugin(manifest)
    try {
      val matchError = assertThrows(RuntimeException::class.java) {
        plugin.withRpcClient { it.matchField(PluginV2.MatchFieldRequest.newBuilder().build()) }
      }
      assertTrue(
        matchError.message?.contains("does not define a global 'match_field' function") == true,
        "unexpected error: ${matchError.message}"
      )

      val generateError = assertThrows(RuntimeException::class.java) {
        plugin.withRpcClient { it.generateField(PluginV2.GenerateFieldRequest.newBuilder().build()) }
      }
      assertTrue(
        generateError.message?.contains("does not define a global 'generate_field' function") == true,
        "unexpected error: ${generateError.message}"
      )
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
    }
  }

  @Test
  fun `match_field calls host_match_field for a registered core capability`() {
    // A plugin that owns a content type delegating one value inside it to a standard Pact rule,
    // rather than reimplementing it - the point of the host callbacks (proposal 006 section 7).
    val key = "match_field-calls-host_match_field-for-a-registered-core-capability"
    CatalogueManager.registerCoreEntries(listOf(
      CatalogueEntry(CatalogueEntryType.MATCHER, CatalogueEntryProviderType.CORE, "", key)
    ))
    CoreCapabilityRegistry.registerFieldMatcher(key) { request ->
      PluginV2.MatchFieldResponse.newBuilder()
        .addMismatches(
          PluginV2.ContentMismatch.newBuilder()
            .setMismatch("core matcher saw rule '${request.rule.type}' at ${request.path}")
            .setPath(request.path)
            .setMismatchType(request.mismatchType)
            .build()
        )
        .build()
    }

    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-match-field-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-match-field-test",
      """
        function match_field(request)
          return host_match_field("$key", request)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient {
        it.matchField(creditcardMatchRequest("visa", "4111111111111111", "4012888888881881"))
      }
      assertEquals(1, response.mismatchesCount)
      assertEquals(
        "core matcher saw rule 'creditcard' at $.card.number",
        response.getMismatches(0).mismatch
      )
      assertEquals("body", response.getMismatches(0).mismatchType)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
      CoreCapabilityRegistry.deregisterFieldMatcher(key)
    }
  }

  @Test
  fun `host_match_field surfaces a clear error when the entry is not registered`() {
    val key = "host_match_field-surfaces-a-clear-error-when-the-entry-is-not-registered"
    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-match-field-missing-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-match-field-missing-test",
      """
        function match_field(request)
          return host_match_field("$key", request)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val ex = assertThrows(RuntimeException::class.java) {
        plugin.withRpcClient {
          it.matchField(creditcardMatchRequest(null, "4111111111111111", "4111111111111111"))
        }
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
  fun `generate_field calls host_generate_field for a registered core capability`() {
    val key = "generate_field-calls-host_generate_field-for-a-registered-core-capability"
    CatalogueManager.registerCoreEntries(listOf(
      CatalogueEntry(CatalogueEntryType.GENERATOR, CatalogueEntryProviderType.CORE, "", key)
    ))
    CoreCapabilityRegistry.registerFieldGenerator(key) {
      PluginV2.GenerateFieldResponse.newBuilder().setValue(textField("generated by the host")).build()
    }

    val pluginDir = kotlin.io.path.createTempDirectory("lua-host-generate-field-test").toFile()
    val manifest = hostCallbackManifest(
      pluginDir,
      "host-generate-field-test",
      """
        function generate_field(request)
          return host_generate_field("$key", request)
        end
      """.trimIndent()
    )
    val plugin = LuaPactPlugin(manifest)
    try {
      val response = plugin.withRpcClient {
        it.generateField(creditcardGenerateRequest("visa", "4111111111111111"))
      }
      assertEquals("", response.error)
      assertEquals("generated by the host", response.value.stringValue)
    } finally {
      plugin.shutdown()
      pluginDir.deleteRecursively()
      CoreCapabilityRegistry.deregisterFieldGenerator(key)
    }
  }
}
