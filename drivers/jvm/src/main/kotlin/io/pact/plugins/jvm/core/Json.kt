package io.pact.plugins.jvm.core

import javax.json.JsonNumber
import javax.json.JsonString
import javax.json.JsonValue

fun toInteger(value: JsonValue?) =
  if (value is JsonNumber) value.intValue()
  else null

fun toString(value: JsonValue?) =
  if (value is JsonString) value.string
  else value?.toString()
