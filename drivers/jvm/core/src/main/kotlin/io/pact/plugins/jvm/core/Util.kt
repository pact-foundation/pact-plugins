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
      is JsonValue.Object -> {
        val builder = Struct.newBuilder()
        json.entries.forEach { (key, value) ->
          builder.putFields(key, jsonToValue(value))
        }
        Value.newBuilder().setStructValue(builder.build()).build()
      }
    }
  }

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

  fun structToJson(struct: Struct?): JsonValue {
    return if (struct == null) {
      JsonValue.Null
    } else {
      JsonValue.Object(struct.fieldsMap.mapValues { valueToJson(it.value) }.toMutableMap())
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
