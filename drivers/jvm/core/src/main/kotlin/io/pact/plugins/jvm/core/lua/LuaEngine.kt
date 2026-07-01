package io.pact.plugins.jvm.core.lua

import java.io.File

/**
 * Abstraction over an embedded Lua interpreter used to run plugins written in Lua.
 *
 * This hides the specific JVM Lua binding library (initially `party.iroiro/luajava`, a JNI
 * binding to the real Lua 5.4 C library) behind a small interface, so the binding can be
 * swapped out later (e.g. for a pure-JVM interpreter) without touching [io.pact.plugins.jvm.core.lua.LuaPactPlugin].
 *
 * Values crossing this boundary (function arguments/results) are plain JVM types: `String`,
 * `Double`, `Boolean`, `ByteBuffer` (for binary/string content), `Map<String, Any?>`,
 * `List<Any?>`, or `null`.
 */
interface LuaEngine : AutoCloseable {
  /**
   * Registers a host (JVM) function as a Lua global, callable from Lua scripts by `name`.
   */
  fun registerFunction(name: String, function: (args: List<Any?>) -> Any?)

  /**
   * Adds a directory to the front of Lua's `package.path`, so `require` can find `.lua`
   * files in it.
   */
  fun addPackagePath(directory: File)

  /**
   * Loads and executes a Lua script file (its top-level code, e.g. function definitions).
   */
  fun loadScript(scriptPath: File)

  /**
   * True if a global with the given name is currently defined and is a function.
   */
  fun hasFunction(name: String): Boolean

  /**
   * Calls a global Lua function by name with the given arguments.
   */
  fun callFunction(name: String, args: List<Any?>): Any?
}
