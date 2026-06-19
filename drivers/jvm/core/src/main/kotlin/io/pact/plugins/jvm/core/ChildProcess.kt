package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.json.JsonException
import au.com.dius.pact.core.support.json.JsonParser
import au.com.dius.pact.core.support.json.JsonValue
import io.github.oshai.kotlinlogging.KotlinLogging
import java.io.File
import java.io.StringReader
import java.lang.Thread.sleep
import java.util.concurrent.LinkedBlockingDeque

private val logger = KotlinLogging.logger {}

/**
 * This class manages the running child process for a plugin
 */
open class ChildProcess(
  val pb: ProcessBuilder,
  private val manifest: PactPluginManifest,
  private val instanceId: String
) {
  /**
   * Child process PID
   */
  val pid: Long
    get() = process.pid()

  private lateinit var errorThread: Thread
  private lateinit var ioThread: Thread
  private lateinit var process: Process
  val channel: LinkedBlockingDeque<JsonValue> = LinkedBlockingDeque()

  private fun pluginLogDir(): File {
    val outputDir = System.getenv("PACT_OUTPUT_DIR")
    return if (outputDir != null) {
      File(outputDir, "logs")
    } else {
      val pluginDir = System.getenv("PACT_PLUGIN_DIR")
        ?: (System.getProperty("user.home") + "/.pact/plugins")
      File(pluginDir, "logs")
    }
  }

  private fun openLogFile(): java.io.PrintWriter? {
    val logDir = pluginLogDir()
    return try {
      logDir.mkdirs()
      val logFile = File(logDir, "pact-plugin-${manifest.name}-${instanceId}.log")
      logger.debug { "Plugin stderr for instance $instanceId captured to ${logFile.absolutePath}" }
      java.io.PrintWriter(java.io.FileWriter(logFile, false), true)
    } catch (e: Exception) {
      logger.warn(e) { "Could not create plugin log file in ${logDir.absolutePath}" }
      null
    }
  }

  /**
   * Starts the child process and attach threads to read the standard output and error. Will scan the standard output
   * for the child process startup message.
   */
  open fun start(): ChildProcess {
    process = pb.start()
    logger.debug { "Child process started = ${process.info()}" }
    sleep(100)

    this.ioThread = Thread {
      val bufferedReader = process.inputStream.bufferedReader()
      while (process.isAlive) {
        if (bufferedReader.ready()) {
          val line = bufferedReader.readLine()
          if (line != null) {
            logger.debug { "Plugin ${manifest.name} [${process.pid()}] || $line" }
            if (line.trim().startsWith("{")) {
              logger.debug { "Got JSON message from plugin process" }
              try {
                val json = JsonParser.parseReader(StringReader(line.trim()))
                channel.offer(json)
              } catch (ex: JsonException) {
                logger.debug(ex) { "Failed to parse JSON message, ignoring it" }
              }
            }
          }
        }
      }
    }
    this.errorThread = Thread {
      val logWriter = openLogFile()
      val bufferedReader = process.errorStream.bufferedReader()
      while (process.isAlive) {
        if (bufferedReader.ready()) {
          val line = bufferedReader.readLine()
          if (line != null) {
            logWriter?.println(line)
          }
        }
      }
      logWriter?.close()
    }
    this.ioThread.start()
    this.errorThread.start()

    logger.debug { "Child process started ok" }

    return this
  }

  /**
   * Destroy the child process.
   */
  open fun destroy() {
    process.destroy()
  }
}
