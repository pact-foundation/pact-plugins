package io.pact.plugins.jvm.core

import mu.KLogging
import java.io.StringReader
import java.lang.Thread.sleep
import java.util.concurrent.LinkedBlockingDeque
import javax.json.Json
import javax.json.JsonObject
import javax.json.stream.JsonParsingException

/**
 * This class manages the running child process for a plugin
 */
open class ChildProcess(
  val pb: ProcessBuilder,
  private val manifest: PactPluginManifest
) {
  /**
   * Child process PID
   */
  val pid: Long
    get() = process.pid()

  private lateinit var errorThread: Thread
  private lateinit var ioThread: Thread
  private lateinit var process: Process
  val channel: LinkedBlockingDeque<JsonObject> = LinkedBlockingDeque()

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
              logger.debug("Got JSON message from plugin process")
              try {
                val jsonReader = Json.createReader(StringReader(line.trim()));
                channel.offer(jsonReader.readObject())
              } catch (ex: JsonParsingException) {
                logger.debug(ex) { "Failed to parse JSON message, ignoring it" }
              }
            }
          }
        }
      }
    }
    this.errorThread = Thread {
      val bufferedReader = process.errorStream.bufferedReader()
      while (process.isAlive) {
        if (bufferedReader.ready()) {
          val line = bufferedReader.readLine()
          if (line != null) {
            logger.error { "Plugin ${manifest.name} [${process.pid()}] || $line" }
          }
        }
      }
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

  companion object : KLogging()
}
