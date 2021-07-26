# Plugin drivers

Plugin drivers provide the functionality to find, load and interface to the plugins for the Pact test framework. Each language
with a Pact implementation requires a plugin driver to work with plugins.

Main plugin driver responsibilities:
* The ability to find plugins.
* Load plugins and extract the plugin manifests that describe what the plugin provides.
* Provide a catalogue of features provided by the plugins.
* Provide a messaging bus to facilitate communication between the language implementation and the plugins.
* Manage the plugin lifecycles.

## Locating plugins

Plugins must be stored on the system in a Pact plugins directory, defined either by the `PACT_PLUGIN_DIR` environment 
variable or in the default `$HOME/.pact/plugins`. Each plugin must be in a separate sub-directory and contain a plugin
manifest file `pact-plugin.json`.

## Plugin manifest file

The plugin manifest file describes what the plugin provides and how to load it. It is a JSON file with the following attributes:

| Attribute | Description |
| --------- | ----------- |
| manifestVersion | Version of the manifest file format. Current is 1 |
| pluginInterfaceVersion | Version of the plugin interface the plugin supports. Current is 1 |
| name | Name of the plugin |
| version | Version of the plugin, following the semver format |
| executableType | Executable type of the plugin. Supported types are: exec, dll, ruby, python, node, jvm |
| minimumRequiredVersion | Minimum required version of the runtime/interpreter to run the plugin |
| entryPoint | The main executable for the plugin |
| dependencies | List of system dependencies or plugins required to be able to execute this plugin |

Example of a manifest for a plugin written in Ruby that provides matching CSV files:

```json
{
  "manifestVersion": 1,
  "pluginInterfaceVersion": 1,
  "name": "pact-csv",
  "version": "0.0.0",
  "executableType": "ruby",
  "minimumRequiredVersion": "2.7.2",
  "entryPoint": "main.rb"
}
```

## Getting the port of the plugin GRPC server

When the plugin is started (except for the DLL executable type), the plugin will print a JSON message to its
standard output that contains the port that the plugin GRPC server is running on. The driver needs to poll the
plugin standard output for this message.

The message will have the following attributes:

| Attribute | Description |
| --------- | ----------- |
| port | The port number the GRPC server for the plugin is listening on |
| serverKey | A randomly generated key required to use as a bearer token when communicating with the plugin |

Example:

```json
{"port": 12345, "serverKey": "b37d2d9a9ceb"}
```


