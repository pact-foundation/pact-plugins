package io.pact.plugins.jvm.core

import mu.KLogging
import java.io.StringReader
import java.util.concurrent.LinkedBlockingDeque
import javax.json.Json
import javax.json.JsonObject

class ChildProcess(
  val pb: ProcessBuilder,
  val manifest: PactPluginManifest
) {
  val pid: Long
    get() = process.pid()

  private lateinit var errorThread: Thread
  private lateinit var ioThread: Thread
  private lateinit var process: Process
  val channel: LinkedBlockingDeque<JsonObject> = LinkedBlockingDeque()

  fun start(): ChildProcess {
    process = pb.start()
    logger.debug { "Child process started = ${process.info()}" }

    this.ioThread = Thread {
      val bufferedReader = process.inputStream.bufferedReader()
      while (process.isAlive) {
        if (bufferedReader.ready()) {
          val line = bufferedReader.readLine()
          if (line != null) {
            logger.debug { "Plugin ${manifest.name} [${process.pid()}] $line" }
            if (line.trim().startsWith("{")) {
              logger.debug("Got JSON message from plugin process")
              val jsonReader = Json.createReader(StringReader(line.trim()));
              channel.offer(jsonReader.readObject())
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
            logger.error { "Plugin ${manifest.name} [${process.pid()}] $line" }
          }
        }
      }
    }
    this.ioThread.start()
    this.errorThread.start()

    logger.debug { "Child process started ok" }

    return this
  }

  fun destroy() {
    process.destroy()
  }

  companion object : KLogging()
}
