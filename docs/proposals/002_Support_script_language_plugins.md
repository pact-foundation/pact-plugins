# Support script language plugins (Draft)

Discussion for this proposal: https://github.com/pact-foundation/pact-plugins/discussions/84

## Summary

Allow plugins to be written in a scripting language (examples would be Lua, Python or Javascript).

## Motivation

Current plugins are implemented as executables that start up a gRPC server to communicate. They run as a separate 
process and require a fair amount of overhead to setup. A scripting language approach may be simpler. 

## Details

Scripting languages are executed by an embedded interpreter. This means they are run in the same address space as the
testing framework, so may be easier to debug (doubtful, but, hey). It also means they can have a simpler interface.

Instead of making RPC calls to the plugin process, the plugin just exposes functions that the plugin driver can call. 
These can map quite easily to the gRPC calls make to the exiting plugins.

## Technical details

For a POC of a JWT plugin written in Lua, see the feat/lua-plugins branch in this repository:
* Consumer test https://github.com/pact-foundation/pact-plugins/blob/feat/lua-plugins/examples/jwt/consumer/src/lib.rs
* Lua Plugin https://github.com/pact-foundation/pact-plugins/tree/feat/lua-plugins/plugins/jwt

All the [gRPC calls](https://github.com/pact-foundation/pact-plugins/blob/feat/lua-plugins/proto/plugin.proto#L398) are 
implemented as Lua functions. See [match contents as an example](https://github.com/pact-foundation/pact-plugins/blob/feat/lua-plugins/plugins/jwt/plugin.lua#L104).

## Benefits

* This will allow plugins to be much simpler, and authored by a wider group of people.
* Call back functionality (see the [V2 Plugin Interface proposal](https://github.com/pact-foundation/pact-plugins/blob/main/docs/proposals/001_V2_Plugin_Interface.md#capability-for-plugins-to-use-the-functionality-from-the-calling-framework))
  can be easily implemented as functions that are exposed by the driver to the plugin. With the Lua plugin, [the logger](https://github.com/pact-foundation/pact-plugins/blob/feat/lua-plugins/plugins/jwt/plugin.lua#L11)
  function is an example. 

## Issues with this approach

### System dependencies
Scripting languages require their own set of system dependencies. No plugin is going to be useful on its own. Plugins 
need to be easily installed, and have no dependencies outside their plugin directory, so all dependencies will
need to be bundled with the plugin.

While Lua is quite simple in this regard, Python requires dependencies to be installed in a particular manner (not too 
terrible) and if we allow JavaScript, then people will want to access system functionality. To access files, etc, they will need to
use Node. If they use Node, they will want to use NPM. Using NPM brings in `node_modules`, and all hell then breaks loose.
 
### Authored by a wider group of people
One of the advantages of the current plugin architecture, the people writing the plugins need to be very technical. Thus,
the implemented plugins probably end up being better implemented. If we allow JavaScript, then .....

### JVM support
Pact-JVM currently has no non-JVM dependencies. This means it can run anywhere that a JVM can run. An embedded interpreter
would force Pact-JVM to become system architecture dependant. While there are JVM versions of interpreters (Rhino, Jython),
these may have particular quirks and would force the plugins authors to test their code running with these interpreters.
Also, Pact-JVM would have to expose system functionality (IO, Sockets, etc.) through exported functions.
