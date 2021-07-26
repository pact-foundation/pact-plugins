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

## Init request to the plugin

Once the port has been extracted from the plugin standard output, the driver must send a `InitPluginRequest`
message via GRPC to the plugin. The plugin will respond with a `InitPluginResponse` which will contain all the
catalogue entries for the features that the plugin provides. The driver needs to update its catalogue with these
entries and then publish the updated catalogue to all loaded plugins (including the new one).

## Feature Catalogue

Each entry in the catalogue is keyed based on the following structure: `$providerType/$name?/$type/$key`, where the
different parts are defined by:

| Attribute | Description |
| --------- | ----------- |
| providerType | Denotes an entry from the core Pact framework (`core`) or from a plugin (`plugin`) |
| name | The name of the plugin (omitted for core entries) |
| type | The type of the entry. Valid values are: content-matcher, matcher, mock-server |
| key | Key for the type. It must be unique withing the entries for the plugin. |

For example, a plugin entry for matching CSV bodies would be `plugin/pact-csv/content-matcher/csv`.

### Core catalogue entries

The driver must provide the following entries from the Pact framework:

| Key | Description |
| --- | ----------- |
| `core/mock-server/http-1` | Http/1.1 mock server | 
| `core/mock-server/https-1` | Http/1.1 + TLS mock server | 
| `core/matcher/v2-regex` | V2 spec regex matcher |
| `core/matcher/v2-type` | V2 spec type matcher |
| `core/matcher/v3-number-type` | V3 spec number matcher |
| `core/matcher/v3-integer-type` | V3 spec integer matcher |
| `core/matcher/v3-decimal-type` | V3 spec decimal matcher |
| `core/matcher/v3-date` | V3 spec date matcher |
| `core/matcher/v3-time` | V3 spec time matcher |
| `core/matcher/v3-datetime` | V3 spec DateTime matcher |
| `core/matcher/v2-min-type` | V2 spec minimum type matcher |
| `core/matcher/v2-max-type` | V2 spec maximum type matcher |
| `core/matcher/v2-minmax-type` | V2 spec minimum/maximum type matcher |
| `core/matcher/v3-includes` | V3 spec includes matcher |
| `core/matcher/v3-null` | V3 spec null matcher |
| `core/matcher/v4-equals-ignore-order` | V4 spec ignore array order matcher matcher |
| `core/matcher/v4-min-equals-ignore-order` | V4 spec ignore array order matcher matcher |
| `core/matcher/v4-max-equals-ignore-order` | V4 spec ignore array order matcher matcher |
| `core/matcher/v4-minmax-equals-ignore-order` | V4 spec ignore array order matcher matcher |
| `core/matcher/v3-content-type` | V3 spec content type matcher |
| `core/matcher/v4-array-contains` | V4 spec array contains matcher |
| `core/matcher/v1-equalit` | V1 spec equality matcher |
| `core/content-matcher/xml` | Matcher for XML content types |
| `core/content-matcher/json` | Matcher for JSON content types |
| `core/content-matcher/text` | Matcher for Text content types |
| `core/content-matcher/multipart-form-data` | Matcher for Multipart Form POST content types |
| `core/content-matcher/form-urlencoded` | Matcher for URL-encoded Form POST content types |
