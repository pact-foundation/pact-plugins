package io.pact.plugins.jvm.core

import io.pact.plugin.Plugin
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
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
}
