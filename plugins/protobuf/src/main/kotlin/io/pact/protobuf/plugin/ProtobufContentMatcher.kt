package io.pact.protobuf.plugin

import au.com.dius.pact.core.matchers.BodyItemMatchResult
import au.com.dius.pact.core.matchers.BodyMatchResult
import au.com.dius.pact.core.matchers.BodyMismatch
import au.com.dius.pact.core.matchers.JsonContentMatcher
import au.com.dius.pact.core.matchers.Matchers
import au.com.dius.pact.core.matchers.MatchingContext
import au.com.dius.pact.core.matchers.MismatchFactory
import au.com.dius.pact.core.matchers.generateDiff
import au.com.dius.pact.core.model.constructPath
import au.com.dius.pact.core.support.json.JsonValue
import com.google.protobuf.ByteString
import com.google.protobuf.Descriptors
import com.google.protobuf.DynamicMessage
import mu.KLogging

object ProtobufContentMatcher : KLogging() {
  fun compare(
    expected: DynamicMessage?,
    actual: DynamicMessage?,
    context: MatchingContext
  ): BodyMatchResult {
    return when {
      expected == null -> BodyMatchResult(null, emptyList())
      actual == null -> BodyMatchResult(null, listOf(
        BodyItemMatchResult("$", listOf(
          BodyMismatch( null, null, "Expected message '${expected.descriptorForType.name}'")
        ))
      ))
      else -> BodyMatchResult(null, compareMessage(listOf("$"), expected, actual, context))
    }
  }

  private fun compareMessage(
    path: List<String>,
    expected: DynamicMessage,
    actual: DynamicMessage,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    logger.debug { ">>> compareMessage($path, $expected, $actual)" }
    return if (expected.descriptorForType.name == actual.descriptorForType.name) {
      val result = mutableListOf<BodyItemMatchResult>()

      expected.allFields.forEach { (field, value) ->
        val fieldPath = path + field.name
        if (field.isMapField) {
          result.addAll(compareMapField(path + field.name, field, value as List<DynamicMessage>, actual, context))
        } else if (field.isRepeated) {
          result.addAll(compareRepeatedField(path + field.name, field, value as List<*>, actual, context))
        } else if (!actual.hasField(field)) {
          result.add(BodyItemMatchResult(constructPath(fieldPath), listOf(
            BodyMismatch(field.name, null, "Expected field '${field.name}' but was missing",
              constructPath(fieldPath),
              generateProtoDiff(expected, actual))
          )))
        } else {
          result.addAll(compareField(fieldPath, field, value, actual.getField(field), { generateProtoDiff(expected, actual) }, context))
        }
      }

      if (!context.allowUnexpectedKeys) {
        actual.allFields.forEach { (field, _) ->
          val fieldPath = path + field.name
          if (!field.isRepeated && !expected.hasField(field)) {
            result.add(BodyItemMatchResult(constructPath(fieldPath), listOf(
              BodyMismatch(null, field.name, "Received unexpected field '${field.name}'",
                constructPath(fieldPath),
                generateProtoDiff(expected, actual))
            )))
          }
        }
      }

      result
    } else {
      listOf(BodyItemMatchResult("$", listOf(BodyMismatch(expected, actual,
        "Expected message '${expected.descriptorForType.name}' but got '${actual.descriptorForType.name}'",
        generateProtoDiff(expected, actual)))))
    }
  }

  private fun compareMapField(
    path: List<String>,
    field: Descriptors.FieldDescriptor,
    expectedValues: List<DynamicMessage>,
    actualMessage: DynamicMessage,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    logger.debug { ">>> compareMapField($path, $field, $expectedValues)" }

    val actualValues = actualMessage.getField(field) as List<DynamicMessage>
    return if (expectedValues.isEmpty() && actualValues.isNotEmpty() && !context.allowUnexpectedKeys) {
      listOf(BodyItemMatchResult(constructPath(path),
        listOf(BodyMismatch(expectedValues, actualValues,
          "Expected map field '${field.name}' to be empty but received $actualValues",
          constructPath(path), null))))
    } else {
      val result = mutableListOf<BodyItemMatchResult>()
      // TODO: generate diff of messages unless disabled. Needs a mechanism to disable diffs.
      val generateDiff = { /*generateJsonDiff(expectedValues, actualValues)*/ "" }

      val expectedEntries = expectedValues.associate { message ->
        val fields = message.allFields.mapKeys { it.key.name }
        fields["key"] as String to fields["value"]
      }
      val actualEntries = actualValues.associate { message ->
        val fields = message.allFields.mapKeys { it.key.name }
        fields["key"] as String to fields["value"]
      }

      if (context.matcherDefined(path)) {
        logger.debug { "compareMapField: matcher defined for path $path" }
        for (matcher in context.selectBestMatcher(path).rules) {
          result.addAll(Matchers.compareMaps(path, matcher, expectedEntries, actualEntries, context, generateDiff) {
            p, expected, actual -> compareField(p, field, expected, actual, { "" }, context)
          })
        }
      } else {
        logger.debug { "compareMapField: no matcher defined for path $path" }
        logger.debug { "                   expected keys ${expectedEntries.keys}" }
        logger.debug { "                   actual keys ${actualEntries.keys}" }
        result.addAll(context.matchKeys(path, expectedEntries, actualEntries, generateDiff))
        for ((key, value) in expectedEntries) {
          val p = path + key
          if (actualEntries.containsKey(key)) {
            result.addAll(compareField(p, field, value, actualEntries[key]!!, { "" }, context))
          } else {
            result.add(
              BodyItemMatchResult(constructPath(path),
                listOf(BodyMismatch(value, null,
                  "Expected map field '${field.name}' to have entry '$key', but was missing",
                  constructPath(path), null))
              )
            )
          }
        }
      }

      result
    }
  }

  private fun compareRepeatedField(
    path: List<String>,
    field: Descriptors.FieldDescriptor,
    expectedList: List<*>,
    actualMessage: DynamicMessage,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    logger.debug { ">>> compareRepeatedField($path, $field, $expectedList)" }
    val actualList = actualMessage.getField(field) as List<*>
    val result = mutableListOf<BodyItemMatchResult>()
    val generateDiff = { /*generateJsonDiff(expectedList, actualList)*/ "" }
    if (context.matcherDefined(path)) {
      logger.debug { "compareRepeatedField: Matcher defined for path $path" }
      val ruleGroup = context.selectBestMatcher(path)
      for (matcher in ruleGroup.rules) {
        result.addAll(
          Matchers.compareLists(path, matcher, expectedList, actualList, context, generateDiff, ruleGroup.cascaded) {
              p, expected, actual, c -> compareField(p, field, expected, actual, { "" }, c)
          }
        )
      }
    } else {
      if (expectedList.isEmpty() && actualList.isNotEmpty()) {
        result.add(BodyItemMatchResult(constructPath(path),
          listOf(BodyMismatch(expectedList, actualList,
            "Expected repeated field '${field.name}' to be empty but received $actualList",
            constructPath(path), null))))
      } else {
        result.addAll(
          Matchers.compareListContent(expectedList, actualList, path, context, generateDiff) {
              p, expected, actual, c -> compareField(p, field, expected, actual, { "" }, c)
          }
        )
        if (expectedList.size != actualList.size) {
          result.add(BodyItemMatchResult(constructPath(path), listOf(BodyMismatch(expectedList, actualList,
            "Expected repeated field '${field.name}' to have ${expectedList.size} values but received ${actualList.size} values",
            constructPath(path), null
          ))))
        }
      }
    }
    return result
  }

  private fun compareField(
    path: List<String>,
    field: Descriptors.FieldDescriptor,
    expectedValue: Any?,
    actualValue: Any?,
    diffCallback: () -> String,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    logger.debug { ">>> compareField($path, $field, $expectedValue [${expectedValue?.javaClass}], $actualValue [${actualValue?.javaClass}])" }
    return when (field.type) {
      Descriptors.FieldDescriptor.Type.DOUBLE -> compareValue(path, field, expectedValue as Double?,
        actualValue as Double?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.FLOAT -> compareValue(path, field, expectedValue as Float?,
        actualValue as Float?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.INT64 -> compareValue(path, field, expectedValue as Long?,
        actualValue as Long?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.UINT64 -> compareValue(path, field, expectedValue as Long?,
        actualValue as Long?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.INT32 -> compareValue(path, field, expectedValue as Int?,
        actualValue as Int?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.FIXED64 -> compareValue(path, field, expectedValue as Double?,
        actualValue as Double?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.FIXED32 -> compareValue(path, field, expectedValue as Double?,
        actualValue as Double?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.BOOL -> compareValue(path, field, expectedValue as Double?,
        actualValue as Double?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.STRING -> compareValue(path, field, expectedValue as String?,
        actualValue as String?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.MESSAGE -> {
        val expected = expectedValue as DynamicMessage
        val actual = actualValue as DynamicMessage
        when (field.messageType.fullName) {
          "google.protobuf.BytesValue" -> {
            logger.debug { "Field is a Protobuf BytesValue" }
            val fieldDescriptor = expected.descriptorForType.findFieldByName("value")
            val expectedByteString = expected.getField(fieldDescriptor) as ByteString
            val actualByteString = actual.getField(fieldDescriptor) as ByteString
            compareValue(path, field, expectedByteString.toByteArray(), actualByteString.toByteArray(), diffCallback,
              context)
          }
          "google.protobuf.Struct" -> {
            logger.debug { "Field is a Struct field" }
            JsonContentMatcher.compare(path, structMessageToJson(expected), structMessageToJson(actual), context)
          }
          else -> compareMessage(path, expectedValue, actualValue, context)
        }
      }
      Descriptors.FieldDescriptor.Type.BYTES -> compareValue(path, field, expectedValue as ByteString?,
        actualValue as ByteString?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.UINT32 -> compareValue(path, field, expectedValue as Int?,
        actualValue as Int?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.ENUM -> compareValue(path, field, expectedValue as Descriptors.EnumValueDescriptor,
        actualValue as Descriptors.EnumValueDescriptor, diffCallback, context)
      Descriptors.FieldDescriptor.Type.SFIXED32 -> compareValue(path, field, expectedValue as Int?,
        actualValue as Int?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.SFIXED64 -> compareValue(path, field, expectedValue as Long?,
        actualValue as Long?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.SINT32 -> compareValue(path, field, expectedValue as Int?,
        actualValue as Int?, diffCallback, context)
      Descriptors.FieldDescriptor.Type.SINT64 -> compareValue(path, field, expectedValue as Long?,
        actualValue as Long?, diffCallback, context)
      else -> listOf(BodyItemMatchResult(constructPath(path),
        listOf(BodyMismatch(expectedValue, actualValue, "There is no matcher implemented for ${field.type}",
          constructPath(path), null))))
    }
  }

  private fun structMessageToJson(value: DynamicMessage): JsonValue {
    logger.debug { ">>> structMessageToJson($value)" }
    return JsonValue.Object(value.allFields.entries.associate { (field, value) ->
      val entries = value as List<DynamicMessage>
      logger.debug { ">>> entries: $entries" }
      val keyDescriptor = entries[0].descriptorForType.findFieldByName("key")
      val key = entries[0].getField(keyDescriptor).toString()
      val valueDescriptor = entries[0].descriptorForType.findFieldByName("value")
      val v = entries[0].getField(valueDescriptor) as DynamicMessage
      key to valueMessageToJson(v)
    }.toMutableMap())
  }

  private fun valueMessageToJson(value: DynamicMessage): JsonValue {
    logger.debug { ">>> valueMessageToJson($value)" }
    val fields = value.allFields.entries.associate { (field, v) -> field.name to v }
    return when {
      fields.containsKey("string_value") -> JsonValue.StringValue(fields["string_value"].toString())
      fields.containsKey("number_value") -> JsonValue.Decimal(fields["number_value"] as Number)
      fields.containsKey("bool_value") -> if (fields["bool_value"] as Boolean) { JsonValue.True } else { JsonValue.False }
      fields.containsKey("struct_value") -> structMessageToJson(fields["struct_value"] as DynamicMessage)
      fields.containsKey("list_value") -> {
        val values = fields["list_value"] as List<*>
        JsonValue.Array(values.map { valueMessageToJson(it as DynamicMessage) }.toMutableList())
      }
      else -> JsonValue.Null
    }
  }

  private fun <T> compareValue(
    path: List<String>,
    field: Descriptors.FieldDescriptor,
    expected: T?,
    actual: T?,
    diffCallback: () -> String,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    logger.debug { ">>> compareValue($path, $field, $expected, $actual, $context)" }
    return if (context.matcherDefined(path)) {
      logger.debug { "compareValue: Matcher defined for path $path" }
      listOf(BodyItemMatchResult(constructPath(path),
        Matchers.domatch(context, path, expected, actual, object : MismatchFactory<BodyMismatch> {
          override fun create(expected: Any?, actual: Any?, message: String, path: List<String>): BodyMismatch {
            return BodyMismatch(expected, actual, message, constructPath(path), diffCallback())
          }
        })))
    } else {
      logger.debug { "compareValue: No matcher defined for path $path, using equality" }
      if (expected == actual) {
        listOf(BodyItemMatchResult(constructPath(path), emptyList()))
      } else {
        listOf(BodyItemMatchResult(constructPath(path),
          listOf(BodyMismatch(expected, actual, "Expected '$expected' (${field}) " +
            "but received value '$actual'", constructPath(path), diffCallback()))))
      }
    }
  }

  private fun generateProtoDiff(expected: DynamicMessage, actual: DynamicMessage): String {
    val actualJson = actual.toString()
    val expectedJson = expected.toString()
    return generateDiff(expectedJson, actualJson).joinToString(separator = "\n")
  }
}
