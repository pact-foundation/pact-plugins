package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.json.JsonValue

fun toInteger(value: JsonValue?) = value?.asNumber()?.toInt()
