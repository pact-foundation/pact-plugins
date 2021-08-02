package io.pact.plugins.jvm.core

import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import mu.KLogging
import java.io.IOException
import java.util.jar.JarInputStream

object Utils : KLogging() {
  fun lookupVersion(clazz: Class<*>): String {
    val url = clazz.protectionDomain?.codeSource?.location
    return if (url != null) {
      val openStream = url.openStream()
      try {
        val jarStream = JarInputStream(openStream)
        jarStream.manifest?.mainAttributes?.getValue("Implementation-Version") ?: ""
      } catch (e: IOException) {
        logger.warn(e) { "Could not load manifest" }
        ""
      } finally {
        openStream.close()
      }
    } else {
      ""
    }
  }

  fun <F> handleWith(f: () -> Any?): Result<F, Exception> {
    return try {
      val result = f()
      if (result is Result<*, *>) result as Result<F, Exception> else Ok(result as F)
    } catch (ex: Exception) {
      Err(ex)
    } catch (ex: Throwable) {
      Err(RuntimeException(ex))
    }
  }
}
