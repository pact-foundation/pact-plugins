//! Manages interactions with Pact plugins
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;
use std::process::{Command, Stdio};

use anyhow::anyhow;
use lazy_static::lazy_static;
use log::{debug, max_level, trace, warn};
use sysinfo::{ProcessExt, Signal, System, SystemExt, Pid};

use crate::child_process::ChildPluginProcess;
use crate::plugin_models::{PactPlugin, PactPluginManifest, PluginDependency};
use crate::proto::InitPluginRequest;
use crate::catalogue_manager::{register_plugin_entries, remove_plugin_entries};

lazy_static! {
  static ref PLUGIN_MANIFEST_REGISTER: Mutex<HashMap<String, PactPluginManifest>> = Mutex::new(HashMap::new());
  static ref PLUGIN_REGISTER: Mutex<HashMap<String, PactPlugin>> = Mutex::new(HashMap::new());
}

/// Load the plugin defined by the dependency information. Will first look in the global
/// plugin registry.
pub async fn load_plugin(plugin: &PluginDependency) -> anyhow::Result<PactPlugin> {
  debug!("Loading plugin {:?}", plugin);
  match lookup_plugin(plugin) {
    Some(plugin) => Ok(plugin),
    None => {
      let manifest = load_plugin_manifest(plugin)?;
      initialise_plugin(&manifest).await
    }
  }
}

/// Look up the plugin in the global plugin register
pub fn lookup_plugin(plugin: &PluginDependency) -> Option<PactPlugin> {
  let guard = PLUGIN_REGISTER.lock().unwrap();
  if let Some(version) = &plugin.version {
    guard.get(format!("{}/{}", plugin.name, version).as_str())
      .cloned()
  } else {
    guard.iter()
      .filter(|(_, value)| value.manifest.name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.manifest.version.cmp(&v2.manifest.version))
      .map(|(_, p)| p.clone())
  }
}

/// Return the plugin manifest for the given plugin. Will first look in the global plugin manifest
/// registry.
pub fn load_plugin_manifest(plugin_dep: &PluginDependency) -> anyhow::Result<PactPluginManifest> {
  debug!("Loading plugin manifest for plugin {:?}", plugin_dep);
  match lookup_plugin_manifest(plugin_dep) {
    Some(manifest) => Ok(manifest),
    None => {
      let env_var = env::var_os("PACT_PLUGIN_DIR");
      let plugin_dir = env_var.unwrap_or_default();
      let plugin_dir = plugin_dir.to_string_lossy();
      let plugin_dir = if plugin_dir.is_empty() {
        home::home_dir().map(|dir| dir.join(".pact/plugins"))
      } else {
        PathBuf::from_str(plugin_dir.as_ref()).ok()
      }.ok_or_else(|| anyhow!("No Pact plugin directory was found (in $HOME/.pact/plugins or $PACT_PLUGIN_DIR)"))?;
      debug!("Looking for plugin in {:?}", plugin_dir);
      if plugin_dir.exists() {
        for entry in fs::read_dir(plugin_dir)? {
          let path = entry?.path();
          trace!("Found: {:?}", path);
          if path.is_dir() {
            let manifest_file = path.join("pact-plugin.json");
            if manifest_file.exists() && manifest_file.is_file() {
              debug!("Found plugin manifest: {:?}", manifest_file);
              let file = File::open(manifest_file)?;
              let reader = BufReader::new(file);
              let manifest: PactPluginManifest = serde_json::from_reader(reader)?;
              let version = manifest.version.clone();
              if manifest.name == plugin_dep.name && (plugin_dep.version.is_none() ||
                plugin_dep.version.as_ref().unwrap() == &version) {
                debug!("Parsed plugin manifest: {:?}", manifest);
                let manifest = PactPluginManifest {
                  plugin_dir: path.to_string_lossy().to_string(),
                  .. manifest
                };
                let key = format!("{}/{}", manifest.name, version);
                {
                  let manifest = manifest.clone();
                  let mut guard = PLUGIN_MANIFEST_REGISTER.lock().unwrap();
                  guard.insert(key.clone(), manifest.clone());
                }
                return Ok(manifest);
              }
            }
          }
        }
        Err(anyhow!("Plugin {:?} was not found (in $HOME/.pact/plugins or $PACT_PLUGIN_DIR)", plugin_dep))
      } else {
        Err(anyhow!("Plugin directory {:?} does not exist", plugin_dir))
      }
    }
  }
}

/// Lookup the plugin manifest in the global plugin manifest registry.
pub fn lookup_plugin_manifest(plugin: &PluginDependency) -> Option<PactPluginManifest> {
  let guard = PLUGIN_MANIFEST_REGISTER.lock().unwrap();
  if let Some(version) = &plugin.version {
    let key = format!("{}/{}", plugin.name, version);
    guard.get(&key).cloned()
  } else {
    guard.iter()
      .filter(|(_, value)| value.name == plugin.name)
      .max_by(|(_, v1), (_, v2)| v1.version.cmp(&v2.version))
      .map(|(_, p)| p.clone())
  }
}

async fn initialise_plugin(manifest: &PactPluginManifest) -> anyhow::Result<PactPlugin> {
  match manifest.executable_type.as_str() {
    "exec" => {
      let plugin = start_plugin_process(manifest).await?;
      debug!("Plugin process started OK (port = {}), sending init message", plugin.port());

      let request = InitPluginRequest {
        implementation: "Pact-Rust".to_string(),
        version: "0".to_string()
      };
      let response = plugin.init_plugin(request).await.map_err(|err| {
        plugin.kill();
        anyhow!("Failed to send init request to the plugin - {}", err)
      })?;
      debug!("Got init response {:?} from plugin {}", response, manifest.name);
      register_plugin_entries(manifest, &response.catalogue);
      tokio::task::spawn(async { publish_updated_catalogue() });

      let key = format!("{}/{}", manifest.name, manifest.version);
      {
        let mut guard = PLUGIN_REGISTER.lock().unwrap();
        guard.insert(key, plugin);
      }

      lookup_plugin(&manifest.as_dependency())
        .ok_or_else(|| anyhow!("An unexpected error has occurred"))
    }
    _ => Err(anyhow!("Plugin executable type of {} is not supported", manifest.executable_type))
  }
}

async fn start_plugin_process(manifest: &PactPluginManifest) -> anyhow::Result<PactPlugin> {
  debug!("Starting plugin with manifest {:?}", manifest);
  let mut path = PathBuf::from(manifest.entry_point.clone());
  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(manifest.entry_point.clone());
  }
  debug!("Starting plugin using {:?}", path);
  let log_level = max_level();
  let child = Command::new(path)
    .env("LOG_LEVEL", log_level.as_str())
    .env("RUST_LOG", log_level.as_str())
    .current_dir(manifest.plugin_dir.clone())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
  let child_pid = child.id();
  debug!("Plugin {} started with PID {:?}", manifest.name, child_pid);

  match ChildPluginProcess::new(child, manifest) {
    Ok(child) => Ok(PactPlugin::new(manifest, child)),
    Err(err) => {
      let s = System::new();
      if let Some(process) = s.process(child_pid as Pid) {
        process.kill(Signal::Term);
      } else {
        warn!("Child process with PID {} was not found", child_pid);
      }
      Err(err)
    }
  }
}

/// Shut down all plugin processes
pub fn shutdown_plugins() {
  debug!("Shutting down all plugins");
  let mut guard = PLUGIN_REGISTER.lock().unwrap();
  for plugin in guard.values() {
    debug!("Shutting down plugin {:?}", plugin);
    plugin.kill();
    remove_plugin_entries(&plugin.manifest.name);
  }
  guard.clear()
}

//   override fun invokeContentMatcher(
//     matcher: ContentMatcher,
//     expected: Any,
//     actual: Any,
//     context: Any
//   ): Any? {
//     return when {
//       matcher is CatalogueContentMatcher && matcher.isCore -> {
//         val clazz = Class.forName(matcher.catalogueEntry.values["implementation"]).kotlin
//         val bodyMatcher = clazz.objectInstance ?: clazz.createInstance()
//         try {
//           clazz.memberFunctions.find { it.name == "matchBody" }!!.call(bodyMatcher, expected, actual, context)
//         } catch (e: InvocationTargetException) {
//           throw e.targetException
//         }
//       }
//       matcher is CatalogueContentMatcher -> {
//         val request = Plugin.CompareContentsRequest.newBuilder()
//           .setExpected(Plugin.Body.newBuilder())
//           .setActual(Plugin.Body.newBuilder())
//           .setContext(com.google.protobuf.Struct.parseFrom(
// //            Json.toJson(context).serialise().toByteArray()
//             "".toByteArray()
//           ))
//           .build()
//         PLUGIN_REGISTER[matcher.catalogueEntry.key]!!.stub!!.compareContents(request)
//       }
//       matcher.isCore -> {
//         try {
//           matcher::class.memberFunctions.find { it.name == "matchBody" }!!.call(matcher, expected, actual, context)
//         } catch (e: InvocationTargetException) {
//           throw e.targetException
//         }
//       }
//       else -> throw RuntimeException("Mis-configured content type matcher $matcher")
//     }
//   }
//
//   override fun configureContentMatcherInteraction(
//     matcher: ContentMatcher,
//     contentType: String,
//     bodyConfig: Map<String, Any?>
//   ): Triple<OptionalBody, MatchingRuleCategory?, Generators?> {
//     val builder = com.google.protobuf.Struct.newBuilder()
//     bodyConfig.forEach { (key, value) ->
//       builder.putFields(key, jsonToValue(toJson(value)))
//     }
//     val request = Plugin.ConfigureContentsRequest.newBuilder()
//       .setContentType(contentType)
//       .setContentsConfig(builder)
//       .build()
//     val plugin = lookupPlugin(matcher.pluginName, null) ?:
//       throw PactPluginNotFoundException(matcher.pluginName, null)
//     logger.debug { "Sending configureContents request to plugin ${plugin.manifest}" }
//     val response = plugin.stub!!.configureContents(request)
//     logger.debug { "Got response: $response" }
//     val returnedContentType = ContentType(response.contents.contentType)
//     val body = OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType)
//     val rules = MatchingRuleCategory("body", response.rulesMap.entries.associate { (key, value) ->
//       key to MatchingRuleGroup(value.ruleList.map {
//         MatchingRule.create(it.type, structToJson(it.values))
//       }.toMutableList(), RuleLogic.AND, false)
//     }.toMutableMap())
//     val generators = Generators(mutableMapOf(Category.BODY to response.generatorsMap.mapValues {
//       createGenerator(it.value.type, structToJson(it.value.values))
//     }.toMutableMap()))
//     logger.debug { "body=$body" }
//     logger.debug { "rules=$rules" }
//     logger.debug { "generators=$generators" }
//     return Triple(body, rules, generators)
//   }
//
//   override fun generateContent(
//     contentGenerator: CatalogueContentGenerator,
//     contentType: ContentType,
//     generators: Map<String, Generator>,
//     body: OptionalBody
//   ): OptionalBody {
//     val plugin = lookupPlugin(contentGenerator.catalogueEntry.pluginName, null) ?:
//       throw PactPluginNotFoundException(contentGenerator.catalogueEntry.pluginName, null)
//     val request = Plugin.GenerateContentRequest.newBuilder()
//       .setContents(Plugin.Body.newBuilder()
//         .setContent(BytesValue.newBuilder().setValue(ByteString.copyFrom(body.orEmpty())))
//         .setContentType(contentType.toString()))
//
//     generators.forEach { (key, generator) ->
//       val builder = Struct.newBuilder()
//       generator.toMap(PactSpecVersion.V4).forEach { (key, value) ->
//         builder.putFields(key, jsonToValue(toJson(value)))
//       }
//       val gen = Plugin.Generator.newBuilder()
//         .setType(generator.type)
//         .setValues(builder)
//         .build()
//       request.putGenerators(key, gen)
//     }
//     logger.debug { "Sending generateContent request to plugin ${plugin.manifest}" }
//     val response = plugin.stub!!.generateContent(request.build())
//     logger.debug { "Got response: $response" }
//     val returnedContentType = ContentType(response.contents.contentType)
//     return OptionalBody.body(response.contents.content.value.toByteArray(), returnedContentType)
//   }

fn publish_updated_catalogue() {
  // val requestBuilder = Plugin.Catalogue.newBuilder()
  // CatalogueManager.entries().forEach { (_, entry) ->
  //   requestBuilder.addCatalogue(Plugin.CatalogueEntry.newBuilder()
  //     .setKey(entry.key)
  //     .setType(entry.type.name)
  //     .putAllValues(entry.values)
  //     .build())
  // }
  // val request = requestBuilder.build()
  //
  // PLUGIN_REGISTER.forEach { (_, plugin) ->
  //   plugin.stub?.updateCatalogue(request)
  // }
}
