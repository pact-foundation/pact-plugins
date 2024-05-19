package io.pact.plugins.jvm.core

import jakarta.json.JsonNumber
import jakarta.json.JsonString
import jakarta.json.JsonValue

fun toInteger(value: JsonValue?) =
  if (value is JsonNumber) value.intValue()
  else null

fun toString(value: JsonValue?) =
  if (value is JsonString) value.string
  else value?.toString()
