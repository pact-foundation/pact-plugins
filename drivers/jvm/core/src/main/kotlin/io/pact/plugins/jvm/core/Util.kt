package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.json.JsonValue
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.michaelbull.result.Result
import com.google.protobuf.ListValue
import com.google.protobuf.NullValue
import com.google.protobuf.Struct
import com.google.protobuf.Value
import mu.KLogging
import org.apache.commons.lang3.SystemUtils
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader
import java.nio.file.Path
import java.nio.file.Paths
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

  /**
   * Convert a JSON type into a Protobuf Value
   */
  fun jsonToValue(json: JsonValue): Value {
    return when (json) {
      is JsonValue.Integer -> Value.newBuilder().setNumberValue(json.toBigInteger().toDouble()).build()
      is JsonValue.Decimal -> Value.newBuilder().setNumberValue(json.toBigDecimal().toDouble()).build()
      is JsonValue.StringValue -> Value.newBuilder().setStringValue(json.toString()).build()
      JsonValue.True -> Value.newBuilder().setBoolValue(true).build()
      JsonValue.False -> Value.newBuilder().setBoolValue(false).build()
      JsonValue.Null -> Value.newBuilder().setNullValue(NullValue.NULL_VALUE).build()
      is JsonValue.Array -> Value.newBuilder().setListValue(
        ListValue.newBuilder().addAllValues(json.values.map { jsonToValue(it) }).build()).build()
      is JsonValue.Object -> Value.newBuilder().setStructValue(toProtoStruct(json.entries)).build()
    }
  }

  /**
   * Convert a Protobuf Value into a JSON value
   */
  fun valueToJson(value: Value?): JsonValue {
    return if (value == null) {
      JsonValue.Null
    } else {
      when (value.kindCase) {
        Value.KindCase.NUMBER_VALUE -> JsonValue.Decimal(value.numberValue)
        Value.KindCase.STRING_VALUE -> JsonValue.StringValue(value.stringValue)
        Value.KindCase.BOOL_VALUE -> if (value.boolValue) {
          JsonValue.True
        } else {
          JsonValue.False
        }
        Value.KindCase.STRUCT_VALUE -> JsonValue.Object(value.structValue.fieldsMap
          .mapValues { valueToJson(it.value) }.toMutableMap())
        Value.KindCase.LIST_VALUE -> JsonValue.Array(value.listValue.valuesList.map { valueToJson(it) }.toMutableList())
        else -> JsonValue.Null
      }
    }
  }

  /**
   * Convert a Protobuf Struct into a JSON value
   */
  fun structToJson(struct: Struct?): JsonValue {
    return if (struct == null) {
      JsonValue.Null
    } else {
      JsonValue.Object(struct.fieldsMap.mapValues { valueToJson(it.value) }.toMutableMap())
    }
  }

  /**
   * Convert a map of JSON values to a Protobuf Struct
   */
  fun toProtoStruct(attributes: Map<String, JsonValue>): Struct {
    val builder = Struct.newBuilder()
    attributes.entries.forEach { (key, value) ->
      builder.putFields(key, jsonToValue(value))
    }
    return builder.build()
  }

  /**
   * Looks for a program in the system path using the which/where command
   */
  fun lookForProgramInPath(desiredProgram: String): Result<Path, String> {
    val pb = ProcessBuilder(if (SystemUtils.IS_OS_WINDOWS) "where" else "which", desiredProgram)
    return try {
      val proc = pb.start()
      val errCode = proc.waitFor()
      if (errCode == 0) {
        BufferedReader(InputStreamReader(proc.inputStream)).use { reader ->
          Ok(Paths.get(reader.readLine()))
        }
      } else {
        Err("$desiredProgram not found in in PATH")
      }
    } catch (ex: IOException) {
      logger.error(ex) { "Something went wrong while searching for $desiredProgram - ${ex.message}" }
      Err("Something went wrong while searching for $desiredProgram - ${ex.message}")
    } catch (ex: InterruptedException) {
      logger.error(ex) { "Something went wrong while searching for $desiredProgram - ${ex.message}" }
      Err("Something went wrong while searching for $desiredProgram - ${ex.message}")
    }
  }
}

public fun String?.ifNullOrEmpty(function: () -> String?): String? {
  return if (this.isNullOrEmpty()) {
    function()
  } else {
    this
  }
}
