package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.Result
import io.github.oshai.kotlinlogging.KotlinLogging
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader

private val logger = KotlinLogging.logger {}

object SystemExec {
  fun execute(prog: String, vararg args: String): Result<String, Pair<Int, String>> {
    val pb = ProcessBuilder(prog, *args)
    return try {
      val proc = pb.start()
      val errCode = proc.waitFor()
      if (errCode == 0) {
        BufferedReader(InputStreamReader(proc.inputStream)).use { reader ->
          Result.Ok(reader.readText())
        }
      } else {
        val errorOut = BufferedReader(InputStreamReader(proc.errorStream)).use { reader -> reader.readText() }
        Result.Err(errCode to errorOut)
      }
    } catch (ex: IOException) {
      logger.error(ex) { "Failed to execute $prog - ${ex.message}" }
      Result.Err(-1 to "Failed to execute $prog - ${ex.message}")
    } catch (ex: InterruptedException) {
      logger.error(ex) { "Failed to execute $prog - ${ex.message}" }
      Result.Err(-2 to "Failed to execute $prog - ${ex.message}")
    }
  }
}
