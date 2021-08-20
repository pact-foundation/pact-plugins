package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.Json
import au.com.dius.pact.core.support.isNotEmpty
import io.pact.plugins.jvm.core.Utils.lookupVersion
import mu.KLogging
import org.apache.hc.client5.http.fluent.Request.post
import org.apache.hc.core5.http.ContentType
import org.apache.hc.core5.http.io.entity.StringEntity
import java.util.UUID

object PluginMetrics: KLogging() {
  fun sendMetrics(manifest: PactPluginManifest) {
    Thread {
      val doNotTrack = lookupProperty("pact_do_not_track").ifNullOrEmpty {
        System.getenv("pact_do_not_track")
      }
      if (doNotTrack != "true") {
        DefaultPluginManager.logger.info {
          """
          Please note: we are tracking this plugin load anonymously to gather important usage statistics.
          To disable tracking, set the 'pact_do_not_track' system property or environment variable to 'true'.
          """
        }
        try {
          val osArch = "${lookupProperty("os.name")?.lowercase()}-${lookupProperty("os.arch")?.lowercase()}"
          val entity = mapOf(
            "v" to 1,                                         // Version of the API
            "tid" to "UA-117778936-1",                        // Property ID
            "cid" to UUID.randomUUID().toString(),            // Anonymous Client ID.
            "an" to "pact-plugins-jvm",                       // App name.
            "aid" to "pact-plugins-jvm",                      // App Id
            "av" to lookupVersion(PluginMetrics::class.java), // App version.
            "aip" to true,                                    // Anonymise IP address
            "ds" to "pact-plugins-jvm",                       // Data source
            "cd1" to "pact-plugins-jvm",                      // Custom Dimension 1: library
            "cd2" to lookupContext(),                         // Custom Dimension 2: context
            "cd3" to osArch,                                  // Custom Dimension 3: osarch
            "cd4" to manifest.name,                           // Custom Dimension 4: plugin_name
            "cd5" to manifest.version                         // Custom Dimension 5: plugin_version
          )
          val stringEntity = StringEntity(Json.toJson(entity).serialise(), ContentType.APPLICATION_JSON)
          val response = post("https://www.google-analytics.com/collect")
            .body(stringEntity)
            .execute()
            .returnResponse()
          if (response.code > 299) {
            logger.debug("Got response from metrics: ${response.code} ${response.reasonPhrase}")
          }
        } catch (ex: Exception) {
          logger.debug(ex) { "Failed to send plugin load metrics" }
        }
      }
    }.run()
  }

  private fun lookupProperty(name: String): String? = System.getProperty(name)

  private fun lookupEnv(name: String): String? = System.getenv(name)

  private fun lookupContext(): String {
    return if (CIs.any { lookupEnv(it).isNotEmpty() }) {
      "CI"
    } else {
      "unknown"
    }
  }

  private val CIs = listOf(
    "CI",
    "CONTINUOUS_INTEGRATION",
    "BSTRUSE_BUILD_DIR",
    "APPVEYOR",
    "BUDDY_WORKSPACE_URL",
    "BUILDKITE",
    "CF_BUILD_URL",
    "CIRCLECI",
    "CODEBUILD_BUILD_ARN",
    "CONCOURSE_URL",
    "DRONE",
    "GITLAB_CI",
    "GO_SERVER_URL",
    "JENKINS_URL",
    "PROBO_ENVIRONMENT",
    "SEMAPHORE",
    "SHIPPABLE",
    "TDDIUM",
    "TEAMCITY_VERSION",
    "TF_BUILD",
    "TRAVIS",
    "WERCKER_ROOT"
  )
}

