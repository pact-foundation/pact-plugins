package io.pact.plugins.jvm.core.lua

import party.iroiro.luajava.JFunction
import party.iroiro.luajava.Lua
import party.iroiro.luajava.Lua.LuaType
import party.iroiro.luajava.LuaException
import party.iroiro.luajava.lua54.Lua54
import java.io.File

/**
 * [LuaEngine] implementation backed by `party.iroiro/luajava`, a JNI binding to the real Lua
 * 5.4 C library.
 *
 * All interaction with the underlying [Lua] instance goes through its raw stack-based API
 * (`push`/`pCall`/`toObject`) rather than its higher-level `LuaValue`-based convenience API:
 * passing a `Map`/`List` directly as a function-call argument via that convenience API does
 * not perform a deep ("FULL") conversion into a real Lua table (it wraps the object instead,
 * breaking `.field` access from Lua), whereas `push(value, Lua.Conversion.FULL)` on the raw
 * stack does. Using the stack API consistently for both directions avoids that trap.
 */
class LuaJavaEngine : LuaEngine {
  private val lua: Lua = Lua54()

  init {
    lua.openLibraries()
  }

  override fun registerFunction(name: String, function: (args: List<Any?>) -> Any?) {
    lua.push(JFunction { l ->
      val argCount = l.getTop()
      // Read through the Lua instance the call arrived on, not this engine's own: for a call made
      // from a coroutine they are different stacks.
      val args = (1..argCount).map { i -> readValue(l, i) }
      when (val result = function(args)) {
        null -> 0
        else -> {
          l.push(result, Lua.Conversion.FULL)
          1
        }
      }
    })
    lua.setGlobal(name)
  }

  override fun addPackagePath(directory: File, includeDirectoryModules: Boolean) {
    val pkg = lua.get("package")
    val existing = pkg.get("path").toJavaObject() as? String ?: ""
    val newEntries = if (includeDirectoryModules) {
      "${directory.absolutePath}/?.lua;${directory.absolutePath}/?/init.lua"
    } else {
      "${directory.absolutePath}/?.lua"
    }
    pkg.set("path", "$newEntries;$existing")
  }

  override fun loadScript(scriptPath: File) {
    lua.run(scriptPath.readText())
  }

  override fun hasFunction(name: String): Boolean {
    lua.getGlobal(name)
    val result = lua.isFunction(-1)
    lua.pop(1)
    return result
  }

  override fun callFunction(name: String, args: List<Any?>): Any? {
    lua.getGlobal(name)
    if (!lua.isFunction(-1)) {
      lua.pop(1)
      throw LuaException(LuaException.LuaError.RUNTIME, "Lua plugin does not define a global '$name' function")
    }
    for (arg in args) {
      if (arg == null) {
        lua.pushNil()
      } else {
        lua.push(arg, Lua.Conversion.FULL)
      }
    }
    lua.pCall(args.size, 1)
    val result = readValue(lua, -1)
    lua.pop(1)
    return result
  }

  override fun close() {
    lua.close()
  }
}

/**
 * Converts the value at [index] on [lua]'s stack into a plain JVM value.
 *
 * This is `toObject`'s job, but `toObject` returns every Lua number as a `Double`, losing Lua
 * 5.4's integer/float distinction. Callers depend on that distinction: a whole number has to stay
 * whole for the `integer`, `decimal` and `type` matching rules to mean anything, which is why the
 * field-level `FieldValue` has an arm per scalar type in the first place (proposal 006, section 5).
 * The reverse direction needs no such handling - luajava already pushes a `Long` as a Lua integer,
 * so the distinction survives the trip *into* a script.
 *
 * Reading the stack directly rather than post-processing `toObject`'s result also fixes it for a
 * number nested anywhere inside a table, not just a scalar return value.
 */
private fun readValue(lua: Lua, index: Int): Any? = when (lua.type(index)) {
  LuaType.NIL, LuaType.NONE, null -> null
  LuaType.BOOLEAN -> lua.toBoolean(index)
  LuaType.NUMBER -> if (lua.isInteger(index)) lua.toInteger(index) else lua.toNumber(index)
  LuaType.STRING -> lua.toString(index)
  LuaType.TABLE -> readTable(lua, index)
  // Userdata, functions, threads: not values a plugin passes across this boundary, but leave them
  // to luajava rather than dropping them
  else -> lua.toObject(index)
}

/** The key the driver wraps binary (non-text) values under when they cross into Lua. */
private const val BINARY_KEY = "binary"

/**
 * Reads the table at [index] by walking it with `next`.
 *
 * Lua does not distinguish arrays from maps, so a table whose keys are exactly `1..n` becomes a
 * `List` and anything else a `Map` with stringified keys - what callers already expect from the
 * `toObject`-plus-normalise this replaced.
 */
private fun readTable(lua: Lua, index: Int): Any? {
  // `next` needs an absolute index: it pushes and pops around the table, moving a relative one
  val tableIndex = if (index < 0) lua.getTop() + 1 + index else index
  val entries = linkedMapOf<Any?, Any?>()

  lua.pushNil()
  while (lua.next(tableIndex) != 0) {
    // The key is now at -2 and the value at -1. The key is read through readValue rather than
    // `toString`, which would coerce a number key to a string *in place* on the stack and break
    // the `next` that follows.
    val key = readValue(lua, -2)
    entries[key] = if (key == BINARY_KEY && lua.type(-1) == LuaType.STRING) {
      // The driver's convention for a value that is bytes rather than text - a binary message
      // metadata value, or a binary FieldValue - is a `{ binary = "..." }` wrapper (see
      // LuaConversions.fieldValueToLua). Read those bytes as bytes: a Lua string is an arbitrary
      // byte array, and `toString` would truncate it at the first NUL.
      lua.toBuffer(-1)
    } else {
      readValue(lua, -1)
    }
    lua.pop(1)
  }

  val size = entries.size
  val isSequence = size > 0 && (1..size).all { entries.containsKey(it.toLong()) }
  return if (isSequence) {
    (1..size).map { entries[it.toLong()] }
  } else {
    entries.entries.associate { (key, value) -> key.toString() to value }
  }
}
