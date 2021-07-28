package io.pact.plugins.jvm.core

import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import mu.KLogging
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader

object SystemExec: KLogging() {
  fun execute(prog: String, vararg args: String): Result<String, Pair<Int, String>> {
    val pb = ProcessBuilder(prog, *args)
    return try {
      val proc = pb.start()
      val errCode = proc.waitFor()
      if (errCode == 0) {
        BufferedReader(InputStreamReader(proc.inputStream)).use { reader ->
          Ok(reader.readText())
        }
      } else {
        Err(errCode to  proc.errorStream.readAllBytes().toString())
      }
    } catch (ex: IOException) {
      logger.error(ex) { "Failed to execute $prog - ${ex.message}" }
      Err(-1 to "Failed to execute $prog - ${ex.message}")
    } catch (ex: InterruptedException) {
      logger.error(ex) { "Failed to execute $prog - ${ex.message}" }
      Err(-2 to "Failed to execute $prog - ${ex.message}")
    }
  }
}
