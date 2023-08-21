package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.contains
import au.com.dius.pact.core.support.isNotEmpty
import io.pact.plugins.jvm.core.Utils.lookupVersion
import io.github.oshai.kotlinlogging.KLogging
import org.apache.commons.codec.digest.DigestUtils
import org.apache.hc.client5.http.fluent.Request
import org.apache.hc.core5.http.message.BasicNameValuePair
import java.util.UUID
import java.util.concurrent.TimeUnit

object PluginMetrics: KLogging() {

  const val GA_ID = "UA-117778936-1"
  const val GA_URL = "https://www.google-analytics.com/collect"

  fun sendMetrics(manifest: PactPluginManifest) {
    Thread {
      val doNotTrack = lookupProperty("pact_do_not_track").ifNullOrEmpty {
        System.getenv("PACT_DO_NOT_TRACK")
      }.ifNullOrEmpty {
        System.getenv("pact_do_not_track")
      }

      if (doNotTrack != "true") {
        logger.warn {
          """
          Please note: we are tracking this plugin load anonymously to gather important usage statistics.
          To disable tracking, set the 'pact_do_not_track' system property or environment variable to 'true'.
          """
        }
        try {
          val osName = lookupProperty("os.name")?.lowercase().orEmpty()
          val osArch = "$osName-${lookupProperty("os.arch")?.lowercase()}"
          val attributes = mapOf(
            "v" to 1,                                               // Version of the API
            "t" to "event",                                         // Hit type, Specifies the metric is for an event
            "tid" to GA_ID,                                         // Property ID
            "cid" to hostnameHash(osName),                          // Anonymous Client ID.
            "an" to "pact-plugins-jvm",                             // App name.
            "aid" to "pact-plugins-jvm",                            // App Id
            "av" to lookupVersion(PluginMetrics::class.java),       // App version.
            "aip" to true,                                          // Anonymise IP address
            "ds" to "client",                                       // Data source
            "cd2" to lookupContext(),                               // Custom Dimension 2: context
            "cd3" to osArch,                                        // Custom Dimension 3: osarch
            "cd4" to manifest.name,                                 // Custom Dimension 4: plugin_name
            "cd5" to manifest.version,                              // Custom Dimension 5: plugin_version
            "cd7" to lookupProperty("java.runtime.version"),  // Custom Dimension 7: platform_version
            "el" to "Plugin loaded",                                // Event
            "ec" to "Plugin",                                       // Category
            "ea" to "Loaded",                                       // Action
            "ev" to 1                                               // Value
          )
          val entity = attributes
            .filterValues { it != null }
            .map {
              BasicNameValuePair(it.key, it.value.toString())
            }

          logger.debug { "Sending event to GA - $attributes" }
          val response = Request.post(GA_URL)
            .bodyForm(entity)
            .execute()
            .returnResponse()
          if (response.code > 299) {
            logger.debug("Got response from metrics: ${response.code} ${response.reasonPhrase}")
          }
        } catch (ex: Exception) {
          logger.debug(ex) { "Failed to send plugin load metrics" }
        }
      }
    }.start()
  }

  /**
   * This function makes a MD5 hash of the hostname
   */
  private fun hostnameHash(osName: String): String {
    val hostName = if (osName.contains("windows")) {
      lookupEnv("COMPUTERNAME")
    } else {
      lookupEnv("HOSTNAME")
    }
    val hashData = hostName
      .ifNullOrEmpty { execHostnameCommand() }
      .ifNullOrEmpty { UUID.randomUUID().toString() }

    return DigestUtils(DigestUtils.getMd5Digest()).digestAsHex(hashData!!.toByteArray())
  }

  private fun execHostnameCommand(): String? {
    val pb = ProcessBuilder("hostname").start()
    pb.waitFor(500, TimeUnit.SECONDS)
    return if (pb.exitValue() == 0) {
      pb.inputStream.bufferedReader().readLine().trim()
    } else {
      // Host name process failed
      null
    }
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

