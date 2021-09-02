package io.pact.protobuf.plugin

import au.com.dius.pact.core.matchers.BodyItemMatchResult
import au.com.dius.pact.core.matchers.BodyMatchResult
import au.com.dius.pact.core.matchers.BodyMismatch
import au.com.dius.pact.core.matchers.BodyMismatchFactory
import au.com.dius.pact.core.matchers.Matchers
import au.com.dius.pact.core.matchers.MatchingContext
import au.com.dius.pact.core.matchers.MismatchFactory
import au.com.dius.pact.core.matchers.generateDiff
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
    return if (expected.descriptorForType.name == actual.descriptorForType.name) {
      val result = mutableListOf<BodyItemMatchResult>()

      expected.allFields.forEach { (field, value) ->
        val fieldPath = path + field.name
        if (!actual.hasField(field)) {
          result.add(BodyItemMatchResult(fieldPath.joinToString("."), listOf(
            BodyMismatch(field.name, null, "Expected field ${field.name} but was missing",
              fieldPath.joinToString("."),
              generateProtoDiff(expected, actual))
          )))
        } else {
          result.addAll(compareField(fieldPath, field, value, actual.getField(field), { generateProtoDiff(expected, actual) }, context))
        }
      }

      if (!context.allowUnexpectedKeys) {
        actual.allFields.forEach { (field, value) ->
          val fieldPath = path + field.name
          if (!expected.hasField(field)) {
            result.add(BodyItemMatchResult(fieldPath.joinToString("."), listOf(
              BodyMismatch(null, field.name, "Received unexpected field ${field.name}",
                fieldPath.joinToString("."),
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

  private fun compareField(
    path: List<String>,
    field: Descriptors.FieldDescriptor,
    expectedValue: Any?,
    actualValue: Any?,
    diffCallback: () -> String,
    context: MatchingContext
  ): List<BodyItemMatchResult> {
    return if (field.isRepeated) {
      TODO()
    } else {
      when (field.type) {
        Descriptors.FieldDescriptor.Type.DOUBLE -> compareValue(
          path,
          field,
          expectedValue as Double?,
          actualValue as Double?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.FLOAT -> compareValue(
          path,
          field,
          expectedValue as Float?,
          actualValue as Float?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.INT64 -> compareValue(
          path,
          field,
          expectedValue as Long?,
          actualValue as Long?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.UINT64 -> compareValue(
          path,
          field,
          expectedValue as Long?,
          actualValue as Long?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.INT32 -> compareValue(
          path,
          field,
          expectedValue as Int?,
          actualValue as Int?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.FIXED64 -> compareValue(
          path,
          field,
          expectedValue as Double?,
          actualValue as Double?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.FIXED32 -> compareValue(
          path,
          field,
          expectedValue as Double?,
          actualValue as Double?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.BOOL -> compareValue(
          path,
          field,
          expectedValue as Double?,
          actualValue as Double?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.STRING -> compareValue(
          path,
          field,
          expectedValue as String?,
          actualValue as String?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.GROUP -> TODO()
        Descriptors.FieldDescriptor.Type.MESSAGE -> TODO()
        Descriptors.FieldDescriptor.Type.BYTES -> TODO()
        Descriptors.FieldDescriptor.Type.UINT32 -> compareValue(
          path,
          field,
          expectedValue as Int?,
          actualValue as Int?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.ENUM -> TODO()
        Descriptors.FieldDescriptor.Type.SFIXED32 -> compareValue(
          path,
          field,
          expectedValue as Int?,
          actualValue as Int?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.SFIXED64 -> compareValue(
          path,
          field,
          expectedValue as Long?,
          actualValue as Long?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.SINT32 -> compareValue(
          path,
          field,
          expectedValue as Int?,
          actualValue as Int?,
          diffCallback,
          context
        )
        Descriptors.FieldDescriptor.Type.SINT64 -> compareValue(
          path,
          field,
          expectedValue as Long?,
          actualValue as Long?,
          diffCallback,
          context
        )
        else -> TODO()
      }
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
    return if (context.matcherDefined(path)) {
      logger.debug { "compareValue: Matcher defined for path $path" }
      listOf(BodyItemMatchResult(path.joinToString("."),
        Matchers.domatch(context, path, expected, actual, object : MismatchFactory<BodyMismatch> {
          override fun create(expected: Any?, actual: Any?, message: String, path: List<String>): BodyMismatch {
            return BodyMismatch(expected, actual, message, path.joinToString("."), diffCallback())
          }
        })))
    } else {
      logger.debug { "compareValue: No matcher defined for path $path, using equality" }
      if (expected == actual) {
        listOf(BodyItemMatchResult(path.joinToString("."), emptyList()))
      } else {
        listOf(BodyItemMatchResult(path.joinToString("."),
          listOf(BodyMismatch(expected, actual, "Expected $expected (${field}) " +
            "but received $actual", path.joinToString("."), diffCallback()))))
      }
    }
  }

  private fun generateProtoDiff(expected: DynamicMessage, actual: DynamicMessage): String {
    val actualJson = actual.toString()
    val expectedJson = expected.toString()
    return generateDiff(expectedJson, actualJson).joinToString(separator = "\n")
  }
}
