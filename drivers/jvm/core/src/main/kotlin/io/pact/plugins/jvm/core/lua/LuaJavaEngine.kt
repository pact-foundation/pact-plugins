package io.pact.plugins.jvm.core.lua

import party.iroiro.luajava.JFunction
import party.iroiro.luajava.Lua
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
      val args = (1..argCount).map { i -> normalize(l.toObject(i)) }
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
    val result = normalize(lua.toObject(-1))
    lua.pop(1)
    return result
  }

  override fun close() {
    lua.close()
  }

  /**
   * `toObject` converts a Lua array-like table (keys `1.0, 2.0, ..., n.0`) into a
   * `Map<Double, Any?>` rather than a `List`, since Lua does not distinguish arrays from
   * maps. Recursively normalises such sequential maps into `List`s, and stringifies other
   * map keys, so callers can treat results as ordinary JSON-shaped Kotlin values.
   */
  private fun normalize(value: Any?): Any? = when (value) {
    is Map<*, *> -> {
      val size = value.size
      val isSequence = size > 0 && (1..size).all { i -> value.containsKey(i.toDouble()) }
      if (isSequence) {
        (1..size).map { i -> normalize(value[i.toDouble()]) }
      } else {
        value.entries.associate { (k, v) -> k.toString() to normalize(v) }
      }
    }
    is List<*> -> value.map { normalize(it) }
    else -> value
  }
}
