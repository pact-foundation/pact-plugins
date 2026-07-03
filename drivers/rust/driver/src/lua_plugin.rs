//! Support for Pact plugins written in Lua.
//!
//! A Lua plugin is loaded as an embedded [`mlua`] interpreter running in the driver's own
//! process (`executableType: "lua"` in `pact-plugin.json`), instead of a separate child
//! process speaking gRPC. The plugin script must define these global functions:
//!
//! - `init(implementation, version) -> table` - returns an array of catalogue entries,
//!   each shaped as `{ entryType = "CONTENT_MATCHER", key = "...", values = { ... } }`.
//! - `configure_interaction(content_type, config) -> table` - see [`PluginInstance::configure_interaction`].
//! - `match_contents(request) -> table` - see [`PluginInstance::compare_contents`].
//! - `generate_content(contents, generators, test_mode)` (optional) - see [`PluginInstance::generate_content`].
//! - `update_catalogue(catalogue)` (optional) - see [`PluginInstance::update_catalogue`].
//!
//! A Lua plugin that registers a `TRANSPORT` catalogue entry (instead of, or as well as, a
//! `CONTENT_MATCHER`/`CONTENT_GENERATOR` one) must also define these functions. The plugin
//! itself is responsible for whatever the transport actually requires (opening sockets,
//! making outbound calls, etc.) - the driver only calls these functions at the right points
//! in the test lifecycle, exactly as it would over gRPC for an `exec` plugin:
//!
//! - `start_mock_server(request) -> table` - see [`PluginInstance::start_mock_server`] /
//!   [`PluginInstance::start_mock_server_v2`].
//! - `shutdown_mock_server(server_key) -> table` - see [`PluginInstance::shutdown_mock_server`].
//! - `get_mock_server_results(server_key) -> table` - see [`PluginInstance::get_mock_server_results`].
//! - `prepare_interaction_for_verification(request) -> table` - see
//!   [`PluginInstance::prepare_interaction_for_verification`] /
//!   [`PluginInstance::prepare_interaction_for_verification_v2`].
//! - `verify_interaction(request) -> table` - see [`PluginInstance::verify_interaction`] /
//!   [`PluginInstance::verify_interaction_v2`].
//!
//! Each of these is called with either a V1-shaped or a V2-shaped request table, never both,
//! depending on the plugin's own `pluginInterfaceVersion` in its manifest - the same static,
//! per-instance choice the driver makes for gRPC plugins (see `plugin_manager.rs`).

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use async_trait::async_trait;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value, Variadic};
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPublicKey, LineEnding};
use rsa::{Pkcs1v15Sign, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha512};
use tracing::debug;

use crate::plugin_models::{
  PactPluginManifest, PactPluginRpc, PluginInitRequest, PluginInitResponse, PluginInstance,
};
use crate::proto::*;
use crate::proto_v2;
use crate::utils::{proto_struct_to_json, proto_value_to_json, to_proto_struct, to_proto_value};

/// A running Lua plugin instance. Each instance owns its own embedded Lua VM.
pub struct LuaPactPlugin {
  runtime: Arc<Mutex<Lua>>,
  manifest: PactPluginManifest,
  instance_id: String,
  plugin_capabilities: Vec<String>,
}

impl std::fmt::Debug for LuaPactPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("LuaPactPlugin")
      .field("manifest", &self.manifest)
      .field("instance_id", &self.instance_id)
      .field("plugin_capabilities", &self.plugin_capabilities)
      .finish()
  }
}

/// Start a Lua plugin: resolve the entry point script, create a Lua VM, register the host
/// functions the plugin can call, and load (execute) the script.
pub(crate) fn start_lua_plugin(
  manifest: &PactPluginManifest,
  instance_id: String,
) -> anyhow::Result<LuaPactPlugin> {
  let script_path = resolve_entry_point(manifest)?;
  debug!("Loading Lua plugin {} from {:?}", manifest.name, script_path);

  let log = Arc::new(LuaPluginLog::open(&manifest.name, &instance_id));
  let lua = Lua::new();
  set_package_path(&lua, manifest)?;
  add_luarocks_path(&lua, manifest)?;
  register_host_functions(&lua, &manifest.name, &log)?;
  load_script(&lua, &script_path)?;

  Ok(LuaPactPlugin {
    runtime: Arc::new(Mutex::new(lua)),
    manifest: manifest.clone(),
    instance_id,
    plugin_capabilities: vec![],
  })
}

impl LuaPactPlugin {
  /// Set the capabilities negotiated for this plugin instance (called once, after the init
  /// handshake, before the instance is shared behind an `Arc`).
  pub(crate) fn set_plugin_capabilities(&mut self, capabilities: Vec<String>) {
    self.plugin_capabilities = capabilities;
  }
}

/// Captures a Lua plugin's diagnostic output (`print` and `logger()` calls) into the same
/// per-instance log file a gRPC plugin's stderr is captured to -
/// `<pact-dir>/logs/pact-plugin-<name>-<instance_id>.log` (see
/// `child_process::open_plugin_log_file`) - so operators don't need to know which kind of
/// plugin they're looking at to find its log. A Lua plugin runs embedded in the driver's own
/// process, so without this its `print` output would otherwise go straight to the driver's
/// own real stdout, mixed in with everything else.
struct LuaPluginLog {
  file: Mutex<Option<File>>,
}

impl LuaPluginLog {
  fn open(plugin_name: &str, instance_id: &str) -> Self {
    LuaPluginLog {
      file: Mutex::new(crate::child_process::open_plugin_log_file(plugin_name, instance_id)),
    }
  }

  fn write_line(&self, line: &str) {
    if let Ok(mut guard) = self.file.lock()
      && let Some(file) = guard.as_mut() {
      let _ = writeln!(file, "{}", line);
      let _ = file.flush();
    }
  }
}

fn resolve_entry_point(manifest: &PactPluginManifest) -> anyhow::Result<PathBuf> {
  let entry_point = PathBuf::from(&manifest.entry_point);
  let path = if entry_point.is_absolute() && entry_point.exists() {
    entry_point
  } else {
    PathBuf::from(&manifest.plugin_dir).join(&manifest.entry_point)
  };
  if !path.exists() {
    return Err(anyhow!("Lua plugin entry point {:?} does not exist", path));
  }
  Ok(path)
}

/// Adds the plugin's own directory (not the entry point script's directory, which may be a
/// subdirectory of it if `entryPoint` is a nested path) to `package.path`, matching the JVM
/// driver's `LuaPactPlugin.kt`, which always uses `manifest.pluginDir` for the same purpose.
fn set_package_path(lua: &Lua, manifest: &PactPluginManifest) -> anyhow::Result<()> {
  let plugin_dir = PathBuf::from(&manifest.plugin_dir);
  let package: Table = lua.globals().get("package")?;
  let existing: String = package.get("path").unwrap_or_default();
  let new_path = format!(
    "{}/?.lua;{}/?/init.lua;{}",
    plugin_dir.to_string_lossy(), plugin_dir.to_string_lossy(), existing
  );
  package.set("path", new_path)?;
  Ok(())
}

/// The Lua version this driver embeds (fixed by mlua's `lua54` feature) - also the version
/// segment LuaRocks uses in its per-version tree layout (e.g. `share/lua/5.4/`).
const LUAROCKS_LUA_VERSION: &str = "5.4";

/// Makes pure-Lua packages installed via `luarocks` available to `require`, so a plugin can
/// depend on rocks instead of vendoring every third-party library it uses.
///
/// LuaRocks installs modules under `<rocks_dir>/share/lua/<version>/`, where `<rocks_dir>`
/// defaults to `~/.luarocks` (its standard per-user tree) but can be a system tree or a
/// custom prefix if the user configured LuaRocks differently. A plugin can override the
/// directory this driver looks in via a `luaRocksDir` key in the manifest's `pluginConfig`.
/// Only the `share/lua` (pure Lua) path is added - packages with compiled C extensions
/// (under `lib/lua`) are not supported.
fn add_luarocks_path(lua: &Lua, manifest: &PactPluginManifest) -> anyhow::Result<()> {
  let configured = manifest.plugin_config.get("luaRocksDir").and_then(|v| v.as_str());
  let rocks_dir = match configured {
    Some(dir) => PathBuf::from(dir),
    None => match home::home_dir() {
      Some(home) => home.join(".luarocks"),
      None => return Ok(()),
    },
  };

  let lua_dir = rocks_dir.join("share").join("lua").join(LUAROCKS_LUA_VERSION);
  if !lua_dir.exists() {
    if configured.is_some() {
      debug!(
        "Configured luaRocksDir '{}' does not have a share/lua/{} directory, ignoring",
        rocks_dir.display(), LUAROCKS_LUA_VERSION
      );
    }
    return Ok(());
  }

  let package: Table = lua.globals().get("package")?;
  let existing: String = package.get("path").unwrap_or_default();
  let new_path = format!(
    "{}/?.lua;{}/?/init.lua;{}",
    lua_dir.to_string_lossy(), lua_dir.to_string_lossy(), existing
  );
  package.set("path", new_path)?;
  debug!("Added LuaRocks path {:?} for plugin {}", lua_dir, manifest.name);
  Ok(())
}

fn load_script(lua: &Lua, script_path: &Path) -> anyhow::Result<()> {
  let script = std::fs::read_to_string(script_path)?;
  lua
    .load(script)
    .set_name(script_path.to_string_lossy().to_string())
    .exec()
    .map_err(|err| anyhow!("Failed to load Lua plugin script {:?} - {}", script_path, err))?;
  Ok(())
}

/// Registers the host (Rust) functions that a Lua plugin script can call: a logger, and the
/// RSA/base64 primitives needed by the JWT plugin (Lua has no crypto standard library).
fn register_host_functions(lua: &Lua, plugin_name: &str, log: &Arc<LuaPluginLog>) -> anyhow::Result<()> {
  let globals = lua.globals();

  let name = plugin_name.to_string();
  let logger_log = log.clone();
  globals.set(
    "logger",
    lua.create_function(move |_, message: String| {
      debug!(plugin = name.as_str(), "{}", message);
      logger_log.write_line(&message);
      Ok(())
    })?,
  )?;

  // Redirects Lua's built-in `print` (its "stdout") into the same per-instance log file, so
  // it doesn't leak into the driver's own real stdout - see `LuaPluginLog`.
  let print_log = log.clone();
  globals.set(
    "print",
    lua.create_function(move |lua, args: Variadic<Value>| {
      let tostring: Function = lua.globals().get("tostring")?;
      let mut parts = Vec::with_capacity(args.len());
      for arg in args.iter() {
        parts.push(tostring.call::<String>(arg.clone())?);
      }
      print_log.write_line(&parts.join("\t"));
      Ok(())
    })?,
  )?;

  globals.set(
    "rsa_sign",
    lua.create_function(|_, (data, key): (mlua::String, String)| {
      let private_key = RsaPrivateKey::from_pkcs1_pem(&key).map_err(mlua::Error::external)?;
      let digest = Sha512::digest(data.as_bytes().as_ref());
      let signature = private_key
        .sign(Pkcs1v15Sign::new::<Sha512>(), &digest)
        .map_err(mlua::Error::external)?;
      Ok(base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        signature,
      ))
    })?,
  )?;

  globals.set(
    "rsa_public_key",
    lua.create_function(|_, key: String| {
      let private_key = RsaPrivateKey::from_pkcs1_pem(&key).map_err(mlua::Error::external)?;
      let public_key = RsaPublicKey::from(&private_key);
      let pem = public_key
        .to_pkcs1_pem(LineEnding::LF)
        .map_err(mlua::Error::external)?;
      Ok(pem)
    })?,
  )?;

  globals.set(
    "rsa_validate",
    lua.create_function(|_, (token_parts, algorithm, key): (Vec<String>, String, String)| {
      if algorithm != "RS512" {
        return Err(mlua::Error::RuntimeError(format!(
          "Unsupported JWT algorithm '{}': only RS512 is supported",
          algorithm
        )));
      }
      if token_parts.len() != 3 {
        return Err(mlua::Error::RuntimeError(
          "Expected a 3 part JWT token (header, payload, signature)".to_string(),
        ));
      }

      let public_key = match RsaPublicKey::from_pkcs1_pem(&key) {
        Ok(key) => key,
        Err(_) => return Ok(false),
      };
      let signature = match decode_base64_lenient(&token_parts[2]) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(false),
      };
      let base_token = format!("{}.{}", token_parts[0], token_parts[1]);
      let digest = Sha512::digest(base_token.as_bytes());
      Ok(
        public_key
          .verify(Pkcs1v15Sign::new::<Sha512>(), &digest, &signature)
          .is_ok(),
      )
    })?,
  )?;

  globals.set(
    "b64_decode_no_pad",
    lua.create_function(|lua, data: String| {
      let bytes = decode_base64_lenient(&data).map_err(mlua::Error::external)?;
      lua.create_string(&bytes)
    })?,
  )?;

  Ok(())
}

/// Decode base64 (URL-safe), trying the padded then the un-padded alphabet.
fn decode_base64_lenient(data: &str) -> anyhow::Result<Vec<u8>> {
  use base64::Engine;
  base64::engine::general_purpose::URL_SAFE
    .decode(data)
    .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(data))
    .map_err(|err| anyhow!("Failed to base64 decode value - {}", err))
}

fn call_init(
  lua: &Lua,
  implementation: &str,
  version: &str,
) -> anyhow::Result<Vec<CatalogueEntry>> {
  let init_fn: Function = lua
    .globals()
    .get("init")
    .map_err(|_| anyhow!("Lua plugin does not define a global 'init' function"))?;
  let result: Table = init_fn
    .call((implementation.to_string(), version.to_string()))
    .map_err(|err| anyhow!("Lua init() function failed - {}", err))?;
  lua_table_to_catalogue_entries(result)
}

fn lua_table_to_catalogue_entries(table: Table) -> anyhow::Result<Vec<CatalogueEntry>> {
  let mut entries = vec![];
  for entry in table.sequence_values::<Table>() {
    let entry = entry?;
    let entry_type_str: String = entry.get("entryType")?;
    let key: String = entry.get("key")?;
    let values: Option<HashMap<String, String>> = entry.get("values")?;
    let entry_type = catalogue_entry::EntryType::from_str_name(&entry_type_str)
      .ok_or_else(|| anyhow!("Unknown catalogue entry type '{}'", entry_type_str))?;
    entries.push(CatalogueEntry {
      r#type: entry_type as i32,
      key,
      values: values.unwrap_or_default(),
    });
  }
  Ok(entries)
}

// ---- Body <-> Lua ----

fn content_type_hint_to_str(hint: i32) -> &'static str {
  match body::ContentTypeHint::try_from(hint).unwrap_or(body::ContentTypeHint::Default) {
    body::ContentTypeHint::Default => "DEFAULT",
    body::ContentTypeHint::Text => "TEXT",
    body::ContentTypeHint::Binary => "BINARY",
  }
}

fn str_to_content_type_hint(hint: &str) -> i32 {
  match hint {
    "TEXT" => body::ContentTypeHint::Text as i32,
    "BINARY" => body::ContentTypeHint::Binary as i32,
    _ => body::ContentTypeHint::Default as i32,
  }
}

fn body_to_lua(lua: &Lua, body: &Option<Body>) -> mlua::Result<Value> {
  match body {
    None => Ok(Value::Nil),
    Some(body) => {
      let table = lua.create_table()?;
      table.set("content_type", body.content_type.clone())?;
      match &body.content {
        Some(bytes) => table.set("contents", lua.create_string(bytes)?)?,
        None => table.set("contents", Value::Nil)?,
      }
      table.set("content_type_hint", content_type_hint_to_str(body.content_type_hint))?;
      Ok(Value::Table(table))
    }
  }
}

fn lua_to_body(value: Value) -> anyhow::Result<Option<Body>> {
  match value {
    Value::Nil => Ok(None),
    Value::Table(table) => {
      let content_type: String = table.get("content_type")?;
      let contents: Option<mlua::String> = table.get("contents")?;
      let content_type_hint: Option<String> = table.get("content_type_hint")?;
      Ok(Some(Body {
        content_type,
        content: contents.map(|s| s.as_bytes().to_vec()),
        content_type_hint: content_type_hint
          .map(|h| str_to_content_type_hint(&h))
          .unwrap_or(body::ContentTypeHint::Default as i32),
      }))
    }
    _ => Err(anyhow!("Expected a body table or nil from Lua, got {}", value.type_name())),
  }
}

// ---- Matching rules / generators / plugin configuration <-> Lua ----

fn matching_rules_to_lua(lua: &Lua, rules: &HashMap<String, MatchingRules>) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  for (path, rule_list) in rules {
    let rules_table = lua.create_table()?;
    for rule in &rule_list.rule {
      let rule_table = lua.create_table()?;
      rule_table.set("type", rule.r#type.clone())?;
      if let Some(values) = &rule.values {
        rule_table.set("values", lua.to_value(&proto_struct_to_json(values))?)?;
      }
      rules_table.push(rule_table)?;
    }
    table.set(path.clone(), rules_table)?;
  }
  Ok(table)
}

fn plugin_configuration_to_lua(lua: &Lua, config: &Option<PluginConfiguration>) -> mlua::Result<Value> {
  match config {
    None => Ok(Value::Nil),
    Some(config) => {
      let table = lua.create_table()?;
      if let Some(interaction_configuration) = &config.interaction_configuration {
        table.set(
          "interaction_configuration",
          lua.to_value(&proto_struct_to_json(interaction_configuration))?,
        )?;
      }
      if let Some(pact_configuration) = &config.pact_configuration {
        table.set(
          "pact_configuration",
          lua.to_value(&proto_struct_to_json(pact_configuration))?,
        )?;
      }
      Ok(Value::Table(table))
    }
  }
}

fn lua_to_plugin_configuration(lua: &Lua, value: Option<Value>) -> anyhow::Result<Option<PluginConfiguration>> {
  match value {
    None | Some(Value::Nil) => Ok(None),
    Some(Value::Table(table)) => {
      let interaction_configuration: Option<serde_json::Value> =
        table.get::<Option<Value>>("interaction_configuration")?
          .map(|v| lua.from_value(v))
          .transpose()?;
      let pact_configuration: Option<serde_json::Value> =
        table.get::<Option<Value>>("pact_configuration")?
          .map(|v| lua.from_value(v))
          .transpose()?;
      Ok(Some(PluginConfiguration {
        interaction_configuration: interaction_configuration.map(|v| to_proto_struct(&as_json_map(v))),
        pact_configuration: pact_configuration.map(|v| to_proto_struct(&as_json_map(v))),
      }))
    }
    Some(other) => Err(anyhow!("Expected a plugin_config table or nil from Lua, got {}", other.type_name())),
  }
}

fn as_json_map(value: serde_json::Value) -> HashMap<String, serde_json::Value> {
  match value {
    serde_json::Value::Object(map) => map.into_iter().collect(),
    _ => HashMap::new(),
  }
}

// ---- CompareContents <-> Lua ----

fn compare_request_to_lua(lua: &Lua, request: &CompareContentsRequest) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  table.set("expected", body_to_lua(lua, &request.expected)?)?;
  table.set("actual", body_to_lua(lua, &request.actual)?)?;
  table.set("allow_unexpected_keys", request.allow_unexpected_keys)?;
  table.set("rules", matching_rules_to_lua(lua, &request.rules)?)?;
  table.set(
    "plugin_configuration",
    plugin_configuration_to_lua(lua, &request.plugin_configuration)?,
  )?;
  Ok(table)
}

fn lua_to_compare_response(table: Table) -> anyhow::Result<CompareContentsResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(CompareContentsResponse {
      error,
      type_mismatch: None,
      results: HashMap::new(),
    });
  }

  let type_mismatch: Option<Table> = table.get("type-mismatch")?;
  if let Some(type_mismatch) = type_mismatch {
    let expected: String = type_mismatch.get("expected")?;
    let actual: String = type_mismatch.get("actual")?;
    return Ok(CompareContentsResponse {
      error: String::new(),
      type_mismatch: Some(ContentTypeMismatch { expected, actual }),
      results: HashMap::new(),
    });
  }

  let mismatches: Option<Table> = table.get("mismatches")?;
  let mut results = HashMap::new();
  if let Some(mismatches) = mismatches {
    for pair in mismatches.pairs::<String, Value>() {
      let (path, value) = pair?;
      let mismatch_list = lua_value_to_content_mismatches(&path, value)?;
      if !mismatch_list.is_empty() {
        results.insert(path, ContentMismatches { mismatches: mismatch_list });
      }
    }
  }

  Ok(CompareContentsResponse {
    error: String::new(),
    type_mismatch: None,
    results,
  })
}

/// Stringifies a scalar Lua value (used for the `expected`/`actual` fields of a mismatch
/// table), rather than requiring exactly a Lua string - a claim/header value being compared
/// could just as easily be a number or boolean.
fn lua_scalar_to_string(value: Value) -> anyhow::Result<Option<String>> {
  match value {
    Value::Nil => Ok(None),
    Value::Boolean(b) => Ok(Some(b.to_string())),
    Value::Integer(i) => Ok(Some(i.to_string())),
    Value::Number(n) => Ok(Some(n.to_string())),
    Value::String(s) => Ok(Some(s.to_str()?.to_string())),
    other => Err(anyhow!("Expected a scalar mismatch value from Lua, got {}", other.type_name())),
  }
}

fn lua_value_to_content_mismatches(path: &str, value: Value) -> anyhow::Result<Vec<ContentMismatch>> {
  match value {
    Value::String(s) => Ok(vec![ContentMismatch {
      expected: None,
      actual: None,
      mismatch: s.to_str()?.to_string(),
      path: path.to_string(),
      diff: String::new(),
      mismatch_type: String::new(),
    }]),
    Value::Table(table) => {
      // Either a single mismatch table ({mismatch=..., expected=..., ...}), or a sequence of them / plain strings
      let mismatch_field: Option<String> = table.get("mismatch")?;
      if let Some(mismatch) = mismatch_field {
        // expected/actual can reasonably be non-string Lua values (e.g. a numeric or boolean
        // claim value), so stringify whatever's there rather than requiring exactly a string.
        let expected = lua_scalar_to_string(table.get("expected")?)?;
        let actual = lua_scalar_to_string(table.get("actual")?)?;
        let path_override: Option<String> = table.get("path")?;
        let diff: Option<String> = table.get("diff")?;
        let mismatch_type: Option<String> = table.get("mismatch_type")?;
        Ok(vec![ContentMismatch {
          expected: expected.map(|s| s.into_bytes()),
          actual: actual.map(|s| s.into_bytes()),
          mismatch,
          path: path_override.unwrap_or_else(|| path.to_string()),
          diff: diff.unwrap_or_default(),
          mismatch_type: mismatch_type.unwrap_or_default(),
        }])
      } else {
        let mut result = vec![];
        for entry in table.sequence_values::<Value>() {
          result.extend(lua_value_to_content_mismatches(path, entry?)?);
        }
        Ok(result)
      }
    }
    Value::Nil => Ok(vec![]),
    other => Err(anyhow!("Expected a mismatch string or table from Lua, got {}", other.type_name())),
  }
}

// ---- ConfigureInteraction <-> Lua ----

/// Converts a single Lua interaction-contents table (shaped as
/// `{ contents = <body>, part_name = "...", plugin_config = <table> }`) into an
/// `InteractionResponse`.
fn lua_to_interaction_response(lua: &Lua, table: Table) -> anyhow::Result<InteractionResponse> {
  let contents: Option<Value> = table.get("contents")?;
  let body = match contents {
    Some(value) => lua_to_body(value)?,
    None => None,
  };
  let plugin_config: Option<Value> = table.get("plugin_config")?;
  let part_name: Option<String> = table.get("part_name")?;
  Ok(InteractionResponse {
    contents: body,
    rules: HashMap::new(),
    generators: HashMap::new(),
    message_metadata: None,
    plugin_configuration: lua_to_plugin_configuration(lua, plugin_config)?,
    interaction_markup: String::new(),
    interaction_markup_type: 0,
    part_name: part_name.unwrap_or_default(),
    metadata_rules: HashMap::new(),
    metadata_generators: HashMap::new(),
  })
}

/// Converts the table returned by the Lua `configure_interaction` function, shaped as
/// `{ interactions = { { contents = <body>, part_name = "..." }, ... }, plugin_config = <table> }`,
/// into a `ConfigureInteractionResponse`. `interactions` is always a sequence, even when there
/// is only one interaction (as is the case for a plain body content-matcher like JWT).
fn lua_to_configure_response(lua: &Lua, table: Table) -> anyhow::Result<ConfigureInteractionResponse> {
  let mut interactions = vec![];
  let items: Option<Table> = table.get("interactions")?;
  if let Some(items) = items {
    for entry in items.sequence_values::<Table>() {
      interactions.push(lua_to_interaction_response(lua, entry?)?);
    }
  }

  let plugin_config: Option<Value> = table.get("plugin_config")?;
  Ok(ConfigureInteractionResponse {
    error: String::new(),
    interaction: interactions,
    plugin_configuration: lua_to_plugin_configuration(lua, plugin_config)?,
  })
}

// ---- TRANSPORT plugin support: mock server / verification <-> Lua ----

/// Converts a `google.protobuf.Struct` to a plain Lua value, `nil` if not set.
fn struct_to_lua(lua: &Lua, value: &Option<prost_types::Struct>) -> mlua::Result<Value> {
  match value {
    Some(value) => lua.to_value(&proto_struct_to_json(value)),
    None => Ok(Value::Nil),
  }
}

/// Converts V2 `InteractionContents` (structured per-interaction data sent in place of a whole
/// Pact JSON document) into a Lua table shaped as
/// `{ interaction_type, consumer, provider, plugin_configuration = { interaction_configuration, pact_configuration } }`.
fn interaction_contents_to_lua(lua: &Lua, contents: &proto_v2::InteractionContents) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  table.set("interaction_type", contents.interaction_type.clone())?;
  table.set("consumer", contents.consumer.clone())?;
  table.set("provider", contents.provider.clone())?;
  if let Some(plugin_configuration) = &contents.plugin_configuration {
    let config_table = lua.create_table()?;
    if let Some(interaction_configuration) = &plugin_configuration.interaction_configuration {
      config_table.set("interaction_configuration", lua.to_value(&proto_struct_to_json(interaction_configuration))?)?;
    }
    if let Some(pact_configuration) = &plugin_configuration.pact_configuration {
      config_table.set("pact_configuration", lua.to_value(&proto_struct_to_json(pact_configuration))?)?;
    }
    table.set("plugin_configuration", config_table)?;
  }
  Ok(table)
}

/// V1 `InteractionData` and V2 `InteractionData` are structurally identical (same wire format);
/// converting via an encode/decode round trip lets the rest of this module deal with a single
/// (V1) type, matching the approach `plugin_manager.rs` uses in the other direction (see
/// `to_proto_v2_interaction_data`). Returns an error rather than panicking if the round trip
/// ever fails (it shouldn't, given the identical wire format, but this data originates from a
/// caller-supplied gRPC request, so a decode failure should be a recoverable error, not a
/// crash).
fn v2_interaction_data_to_v1(data: &proto_v2::InteractionData) -> anyhow::Result<InteractionData> {
  use prost::Message;
  InteractionData::decode(data.encode_to_vec().as_slice())
    .map_err(|err| anyhow!("Failed to convert V2 InteractionData to V1 - {}", err))
}

/// Converts request/response metadata to a Lua table. Each value is either a plain Lua value
/// (JSON-like, for a non-binary `MetadataValue`) or a `{ binary = <lua string> }` wrapper table
/// (for a binary `MetadataValue`), so a Lua script can tell the two apart.
fn metadata_to_lua(lua: &Lua, metadata: &HashMap<String, MetadataValue>) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  for (key, value) in metadata {
    let lua_value = match &value.value {
      Some(metadata_value::Value::NonBinaryValue(value)) => lua.to_value(&proto_value_to_json(value))?,
      Some(metadata_value::Value::BinaryValue(bytes)) => {
        let wrapper = lua.create_table()?;
        wrapper.set("binary", lua.create_string(bytes)?)?;
        Value::Table(wrapper)
      }
      None => Value::Nil,
    };
    table.set(key.clone(), lua_value)?;
  }
  Ok(table)
}

/// Converts a Lua metadata table (see [`metadata_to_lua`]) back into `MetadataValue`s.
fn lua_to_metadata(lua: &Lua, table: Option<Table>) -> anyhow::Result<HashMap<String, MetadataValue>> {
  let mut metadata = HashMap::new();
  if let Some(table) = table {
    for pair in table.pairs::<String, Value>() {
      let (key, value) = pair?;
      let binary: Option<mlua::String> = match &value {
        Value::Table(wrapper) => wrapper.get("binary")?,
        _ => None,
      };
      let metadata_value = if let Some(binary) = binary {
        metadata_value::Value::BinaryValue(binary.as_bytes().to_vec())
      } else {
        let json: serde_json::Value = lua.from_value(value)?;
        metadata_value::Value::NonBinaryValue(to_proto_value(&json))
      };
      metadata.insert(key, MetadataValue { value: Some(metadata_value) });
    }
  }
  Ok(metadata)
}

/// Converts `InteractionData` (a request/response body plus metadata) to a Lua table shaped as
/// `{ body = <body table>, metadata = <metadata table> }`, or `nil` if not set.
fn interaction_data_to_lua(lua: &Lua, data: &Option<InteractionData>) -> mlua::Result<Value> {
  match data {
    None => Ok(Value::Nil),
    Some(data) => {
      let table = lua.create_table()?;
      table.set("body", body_to_lua(lua, &data.body)?)?;
      table.set("metadata", metadata_to_lua(lua, &data.metadata)?)?;
      Ok(Value::Table(table))
    }
  }
}

/// Converts a Lua interaction-data table (see [`interaction_data_to_lua`]) back into
/// `InteractionData`, or `None` if the Lua value was `nil`.
fn lua_to_interaction_data(lua: &Lua, value: Option<Value>) -> anyhow::Result<Option<InteractionData>> {
  match value {
    None | Some(Value::Nil) => Ok(None),
    Some(Value::Table(table)) => {
      let body: Option<Value> = table.get("body")?;
      let body = match body {
        Some(value) => lua_to_body(value)?,
        None => None,
      };
      let metadata_table: Option<Table> = table.get("metadata")?;
      Ok(Some(InteractionData {
        body,
        metadata: lua_to_metadata(lua, metadata_table)?,
      }))
    }
    Some(other) => Err(anyhow!("Expected an interaction data table or nil from Lua, got {}", other.type_name())),
  }
}

/// Converts the table returned by the Lua `start_mock_server` function, shaped as either
/// `{ error = "..." }` or `{ details = { key, port, address } }`, into a `StartMockServerResponse`.
fn lua_to_start_mock_server_response(table: Table) -> anyhow::Result<StartMockServerResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(StartMockServerResponse {
      response: Some(start_mock_server_response::Response::Error(error)),
    });
  }

  let details: Option<Table> = table.get("details")?;
  let details = details.ok_or_else(|| {
    anyhow!("Lua start_mock_server() must return either an 'error' or 'details' field")
  })?;
  Ok(StartMockServerResponse {
    response: Some(start_mock_server_response::Response::Details(MockServerDetails {
      key: details.get("key")?,
      port: details.get("port")?,
      address: details.get("address")?,
    })),
  })
}

/// Converts the table returned by the Lua `shutdown_mock_server`/`get_mock_server_results`
/// functions, shaped as `{ ok = bool, results = { { path, error, mismatches = { ... } }, ... } }`,
/// into `MockServerResults`. Reuses [`lua_value_to_content_mismatches`] for each result's
/// `mismatches` field, the same helper `match_contents` responses use.
fn lua_to_mock_server_results(table: Table) -> anyhow::Result<MockServerResults> {
  let ok: bool = table.get::<Option<bool>>("ok")?.unwrap_or(true);
  let mut results = vec![];
  let results_table: Option<Table> = table.get("results")?;
  if let Some(results_table) = results_table {
    for entry in results_table.sequence_values::<Table>() {
      let entry = entry?;
      let path: String = entry.get::<Option<String>>("path")?.unwrap_or_default();
      let error: String = entry.get::<Option<String>>("error")?.unwrap_or_default();
      let mismatches_value: Value = entry.get("mismatches")?;
      results.push(MockServerResult {
        path: path.clone(),
        error,
        mismatches: lua_value_to_content_mismatches(&path, mismatches_value)?,
      });
    }
  }
  Ok(MockServerResults { ok, results })
}

/// Converts the table returned by the Lua `prepare_interaction_for_verification` function,
/// shaped as either `{ error = "..." }` or `{ interaction_data = { body, metadata } }`, into a
/// `VerificationPreparationResponse`.
fn lua_to_verification_preparation_response(
  lua: &Lua,
  table: Table,
) -> anyhow::Result<VerificationPreparationResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(VerificationPreparationResponse {
      response: Some(verification_preparation_response::Response::Error(error)),
    });
  }

  let data: Option<Value> = table.get("interaction_data")?;
  let data = data.ok_or_else(|| {
    anyhow!("Lua prepare_interaction_for_verification() must return either an 'error' or 'interaction_data' field")
  })?;
  let interaction_data = lua_to_interaction_data(lua, Some(data))?
    .unwrap_or_else(|| InteractionData { body: None, metadata: HashMap::new() });
  Ok(VerificationPreparationResponse {
    response: Some(verification_preparation_response::Response::InteractionData(interaction_data)),
  })
}

/// Converts a single Lua verification mismatch (a plain error string, or a mismatch table shaped
/// like a `match_contents` mismatch) into a `VerificationResultItem`.
fn lua_to_verification_result_item(value: Value) -> anyhow::Result<VerificationResultItem> {
  match value {
    Value::String(s) => Ok(VerificationResultItem {
      result: Some(verification_result_item::Result::Error(s.to_str()?.to_string())),
    }),
    Value::Table(table) => {
      let mismatch: Option<String> = table.get("mismatch")?;
      let path: Option<String> = table.get("path")?;
      let expected = lua_scalar_to_string(table.get("expected")?)?;
      let actual = lua_scalar_to_string(table.get("actual")?)?;
      let diff: Option<String> = table.get("diff")?;
      let mismatch_type: Option<String> = table.get("mismatch_type")?;
      Ok(VerificationResultItem {
        result: Some(verification_result_item::Result::Mismatch(ContentMismatch {
          expected: expected.map(|s| s.into_bytes()),
          actual: actual.map(|s| s.into_bytes()),
          mismatch: mismatch.unwrap_or_default(),
          path: path.unwrap_or_default(),
          diff: diff.unwrap_or_default(),
          mismatch_type: mismatch_type.unwrap_or_default(),
        })),
      })
    }
    other => Err(anyhow!("Expected a mismatch string or table from Lua, got {}", other.type_name())),
  }
}

/// Converts the table returned by the Lua `verify_interaction` function, shaped as either
/// `{ error = "..." }` or
/// `{ result = { success, response_data, mismatches = { ... }, output = { ... } } }`, into a
/// `VerifyInteractionResponse`.
fn lua_to_verify_interaction_response(lua: &Lua, table: Table) -> anyhow::Result<VerifyInteractionResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(VerifyInteractionResponse {
      response: Some(verify_interaction_response::Response::Error(error)),
    });
  }

  let result_table: Option<Table> = table.get("result")?;
  let result_table = result_table
    .ok_or_else(|| anyhow!("Lua verify_interaction() must return either an 'error' or 'result' field"))?;

  let success: bool = result_table.get::<Option<bool>>("success")?.unwrap_or(false);
  let response_data: Option<Value> = result_table.get("response_data")?;
  let response_data = lua_to_interaction_data(lua, response_data)?;

  let mut mismatches = vec![];
  let mismatches_value: Option<Value> = result_table.get("mismatches")?;
  if let Some(Value::Table(mismatches_table)) = mismatches_value {
    for entry in mismatches_table.sequence_values::<Value>() {
      mismatches.push(lua_to_verification_result_item(entry?)?);
    }
  }

  let output: Option<Vec<String>> = result_table.get("output")?;

  Ok(VerifyInteractionResponse {
    response: Some(verify_interaction_response::Response::Result(VerificationResult {
      success,
      response_data,
      mismatches,
      output: output.unwrap_or_default(),
    })),
  })
}

#[async_trait]
impl PactPluginRpc for LuaPactPlugin {
  async fn init_plugin(&mut self, request: PluginInitRequest) -> anyhow::Result<PluginInitResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let catalogue = call_init(&lua, &request.implementation, &request.version)?;
    Ok(PluginInitResponse {
      catalogue,
      plugin_capabilities: vec![],
    })
  }
}

#[async_trait]
impl PluginInstance for LuaPactPlugin {
  fn manifest(&self) -> &PactPluginManifest {
    &self.manifest
  }

  fn instance_id(&self) -> &str {
    &self.instance_id
  }

  fn has_capability(&self, capability: &str) -> bool {
    self.plugin_capabilities.iter().any(|c| c == capability)
  }

  async fn compare_contents(
    &self,
    request: CompareContentsRequest,
  ) -> anyhow::Result<CompareContentsResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let match_fn: Function = lua
      .globals()
      .get("match_contents")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'match_contents' function"))?;
    let request_table = compare_request_to_lua(&lua, &request)?;
    let result: Table = match_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua match_contents() function failed - {}", err))?;
    lua_to_compare_response(result)
  }

  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let configure_fn: Function = lua
      .globals()
      .get("configure_interaction")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'configure_interaction' function"))?;
    let config: Value = match &request.contents_config {
      Some(config) => lua.to_value(&proto_struct_to_json(config))?,
      None => Value::Nil,
    };
    let result: Table = configure_fn
      .call((request.content_type.clone(), config))
      .map_err(|err| anyhow!("Lua configure_interaction() function failed - {}", err))?;
    lua_to_configure_response(&lua, result)
  }

  async fn generate_content(
    &self,
    request: GenerateContentRequest,
  ) -> anyhow::Result<GenerateContentResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let generate_fn: Option<Function> = lua.globals().get("generate_content")?;
    match generate_fn {
      None => Ok(GenerateContentResponse {
        contents: request.contents,
      }),
      Some(generate_fn) => {
        let contents = body_to_lua(&lua, &request.contents)?;
        let generators = lua.create_table()?;
        for (path, generator) in &request.generators {
          let generator_table = lua.create_table()?;
          generator_table.set("type", generator.r#type.clone())?;
          if let Some(values) = &generator.values {
            generator_table.set("values", lua.to_value(&proto_struct_to_json(values))?)?;
          }
          generators.set(path.clone(), generator_table)?;
        }
        let test_mode = match generate_content_request::TestMode::try_from(request.test_mode)
          .unwrap_or(generate_content_request::TestMode::Unknown)
        {
          generate_content_request::TestMode::Consumer => "Consumer",
          generate_content_request::TestMode::Provider => "Provider",
          generate_content_request::TestMode::Unknown => "Unknown",
        };
        let result: Value = generate_fn
          .call((contents, generators, test_mode))
          .map_err(|err| anyhow!("Lua generate_content() function failed - {}", err))?;
        Ok(GenerateContentResponse {
          contents: lua_to_body(result)?,
        })
      }
    }
  }

  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let start_fn: Function = lua
      .globals()
      .get("start_mock_server")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'start_mock_server' function"))?;
    let request_table = lua.create_table()?;
    request_table.set("host_interface", request.host_interface)?;
    request_table.set("port", request.port)?;
    request_table.set("tls", request.tls)?;
    request_table.set("pact", request.pact)?;
    request_table.set("test_context", struct_to_lua(&lua, &request.test_context)?)?;
    let result: Table = start_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua start_mock_server() function failed - {}", err))?;
    lua_to_start_mock_server_response(result)
  }

  async fn start_mock_server_v2(
    &self,
    request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let start_fn: Function = lua
      .globals()
      .get("start_mock_server")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'start_mock_server' function"))?;
    let request_table = lua.create_table()?;
    request_table.set("host_interface", request.host_interface)?;
    request_table.set("port", request.port)?;
    request_table.set("tls", request.tls)?;
    let interactions_table = lua.create_table()?;
    for interaction in &request.interactions {
      interactions_table.push(interaction_contents_to_lua(&lua, interaction)?)?;
    }
    request_table.set("interactions", interactions_table)?;
    request_table.set("test_context", struct_to_lua(&lua, &request.test_context)?)?;
    let result: Table = start_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua start_mock_server() function failed - {}", err))?;
    lua_to_start_mock_server_response(result)
  }

  async fn shutdown_mock_server(
    &self,
    request: ShutdownMockServerRequest,
  ) -> anyhow::Result<ShutdownMockServerResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let shutdown_fn: Function = lua
      .globals()
      .get("shutdown_mock_server")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'shutdown_mock_server' function"))?;
    let result: Table = shutdown_fn
      .call(request.server_key)
      .map_err(|err| anyhow!("Lua shutdown_mock_server() function failed - {}", err))?;
    let results = lua_to_mock_server_results(result)?;
    Ok(ShutdownMockServerResponse {
      ok: results.ok,
      results: results.results,
    })
  }

  async fn get_mock_server_results(
    &self,
    request: MockServerRequest,
  ) -> anyhow::Result<MockServerResults> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let results_fn: Function = lua
      .globals()
      .get("get_mock_server_results")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'get_mock_server_results' function"))?;
    let result: Table = results_fn
      .call(request.server_key)
      .map_err(|err| anyhow!("Lua get_mock_server_results() function failed - {}", err))?;
    lua_to_mock_server_results(result)
  }

  async fn prepare_interaction_for_verification(
    &self,
    request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let prepare_fn: Function = lua.globals().get("prepare_interaction_for_verification").map_err(|_| {
      anyhow!("Lua plugin does not define a global 'prepare_interaction_for_verification' function")
    })?;
    let request_table = lua.create_table()?;
    request_table.set("pact", request.pact)?;
    request_table.set("interaction_key", request.interaction_key)?;
    request_table.set("config", struct_to_lua(&lua, &request.config)?)?;
    let result: Table = prepare_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua prepare_interaction_for_verification() function failed - {}", err))?;
    lua_to_verification_preparation_response(&lua, result)
  }

  async fn prepare_interaction_for_verification_v2(
    &self,
    request: proto_v2::VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let prepare_fn: Function = lua.globals().get("prepare_interaction_for_verification").map_err(|_| {
      anyhow!("Lua plugin does not define a global 'prepare_interaction_for_verification' function")
    })?;
    let request_table = lua.create_table()?;
    if let Some(interaction_contents) = &request.interaction_contents {
      request_table.set("interaction_contents", interaction_contents_to_lua(&lua, interaction_contents)?)?;
    }
    request_table.set("config", struct_to_lua(&lua, &request.config)?)?;
    request_table.set("test_context", struct_to_lua(&lua, &request.test_context)?)?;
    let result: Table = prepare_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua prepare_interaction_for_verification() function failed - {}", err))?;
    lua_to_verification_preparation_response(&lua, result)
  }

  async fn verify_interaction(
    &self,
    request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let verify_fn: Function = lua
      .globals()
      .get("verify_interaction")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'verify_interaction' function"))?;
    let request_table = lua.create_table()?;
    request_table.set("interaction_data", interaction_data_to_lua(&lua, &request.interaction_data)?)?;
    request_table.set("config", struct_to_lua(&lua, &request.config)?)?;
    request_table.set("pact", request.pact)?;
    request_table.set("interaction_key", request.interaction_key)?;
    let result: Table = verify_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua verify_interaction() function failed - {}", err))?;
    lua_to_verify_interaction_response(&lua, result)
  }

  async fn verify_interaction_v2(
    &self,
    request: proto_v2::VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let verify_fn: Function = lua
      .globals()
      .get("verify_interaction")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'verify_interaction' function"))?;
    let request_table = lua.create_table()?;
    let interaction_data = request.interaction_data.as_ref()
      .map(v2_interaction_data_to_v1)
      .transpose()?;
    request_table.set("interaction_data", interaction_data_to_lua(&lua, &interaction_data)?)?;
    request_table.set("config", struct_to_lua(&lua, &request.config)?)?;
    if let Some(interaction_contents) = &request.interaction_contents {
      request_table.set("interaction_contents", interaction_contents_to_lua(&lua, interaction_contents)?)?;
    }
    request_table.set("test_context", struct_to_lua(&lua, &request.test_context)?)?;
    let result: Table = verify_fn
      .call(request_table)
      .map_err(|err| anyhow!("Lua verify_interaction() function failed - {}", err))?;
    lua_to_verify_interaction_response(&lua, result)
  }

  async fn update_catalogue(&self, request: Catalogue) -> anyhow::Result<()> {
    let lua = self.runtime.lock().map_err(|_| anyhow!("Lua runtime mutex was poisoned"))?;
    let update_fn: Option<Function> = lua.globals().get("update_catalogue")?;
    if let Some(update_fn) = update_fn {
      let table = lua.create_table()?;
      for entry in &request.catalogue {
        let entry_table = lua.create_table()?;
        let entry_type = catalogue_entry::EntryType::try_from(entry.r#type)
          .unwrap_or(catalogue_entry::EntryType::ContentMatcher);
        entry_table.set("entryType", entry_type.as_str_name())?;
        entry_table.set("key", entry.key.clone())?;
        entry_table.set("values", entry.values.clone())?;
        table.push(entry_table)?;
      }
      update_fn
        .call::<()>(table)
        .map_err(|err| anyhow!("Lua update_catalogue() function failed - {}", err))?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use super::*;

  fn jwt_manifest() -> PactPluginManifest {
    // Deliberately not `.canonicalize()`d: on Windows that returns a `\\?\`-prefixed verbatim
    // path, and the forward slashes `set_package_path`/`add_luarocks_path` append to build
    // Lua's `package.path` aren't auto-translated to `\` under that prefix (unlike a normal
    // path), breaking `require` for every sibling .lua file. A plain absolute path (with
    // unresolved `..` components) resolves fine without it.
    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/jwt");
    assert!(plugin_dir.exists(), "plugins/jwt directory should exist at {:?}", plugin_dir);
    PactPluginManifest {
      plugin_dir: plugin_dir.to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "jwt".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "plugin.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    }
  }

  const PRIVATE_KEY: &str = include_str!("../tests/fixtures/jwt-test-key.pem");

  #[test]
  fn loads_pure_lua_packages_from_a_configured_luarocks_directory() {
    let rocks_root = tempdir::TempDir::new("luarocks-test").unwrap();
    let lua_dir = rocks_root.path().join("share").join("lua").join(LUAROCKS_LUA_VERSION);
    std::fs::create_dir_all(&lua_dir).unwrap();
    std::fs::write(
      lua_dir.join("greeter.lua"),
      r#"return { hello = function() return "hello from luarocks" end }"#,
    ).unwrap();

    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        local greeter = require "greeter"
        GREETER_RESULT = greeter.hello()
      "#,
    ).unwrap();

    let mut plugin_config = HashMap::new();
    plugin_config.insert(
      "luaRocksDir".to_string(),
      serde_json::Value::String(rocks_root.path().to_string_lossy().to_string()),
    );

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "luarocks-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config,
    };

    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().unwrap();
    let result: String = lua.globals().get("GREETER_RESULT").unwrap();
    assert_eq!(result, "hello from luarocks");
  }

  #[test]
  fn loads_a_vendored_directory_style_module_from_the_plugin_directory() {
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    let module_dir = plugin_dir.path().join("greeter");
    std::fs::create_dir_all(&module_dir).unwrap();
    std::fs::write(
      module_dir.join("init.lua"),
      r#"return { hello = function() return "hello from a vendored module" end }"#,
    ).unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        local greeter = require "greeter"
        GREETER_RESULT = greeter.hello()
      "#,
    ).unwrap();

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "vendored-module-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    };

    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().unwrap();
    let result: String = lua.globals().get("GREETER_RESULT").unwrap();
    assert_eq!(result, "hello from a vendored module");
  }

  #[test]
  fn loads_a_vendored_module_when_the_entry_point_is_in_a_subdirectory() {
    // package.path must be rooted at the plugin directory, not the entry point script's own
    // directory, so a vendored module sitting next to a nested entry point still resolves -
    // matching the JVM driver, which always uses `manifest.pluginDir`.
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("greeter.lua"),
      r#"return { hello = function() return "hello from the plugin root" end }"#,
    ).unwrap();
    let src_dir = plugin_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
      src_dir.join("entry.lua"),
      r#"
        local greeter = require "greeter"
        GREETER_RESULT = greeter.hello()
      "#,
    ).unwrap();

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "nested-entry-point-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "src/entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    };

    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().unwrap();
    let result: String = lua.globals().get("GREETER_RESULT").unwrap();
    assert_eq!(result, "hello from the plugin root");
  }

  #[test]
  fn ignores_a_missing_luarocks_directory_instead_of_failing() {
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(plugin_dir.path().join("entry.lua"), "-- no-op").unwrap();

    let mut plugin_config = HashMap::new();
    plugin_config.insert(
      "luaRocksDir".to_string(),
      serde_json::Value::String("/no/such/directory".to_string()),
    );

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "luarocks-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config,
    };

    start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
  }

  #[test]
  fn captures_print_and_logger_output_into_the_per_instance_log_file() {
    let output_dir = tempdir::TempDir::new("lua-plugin-log-test").unwrap();
    // SAFETY: no other test reads/writes PACT_OUTPUT_DIR; matches existing test conventions
    // in this crate for env-var-configured global state (see plugin_manager.rs tests).
    unsafe { std::env::set_var("PACT_OUTPUT_DIR", output_dir.path()); }

    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        print("hello", "world", 42)
        logger("a logger message")
      "#,
    ).unwrap();

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "log-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    };

    start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    unsafe { std::env::remove_var("PACT_OUTPUT_DIR"); }

    let log_path = output_dir.path().join("logs").join("pact-plugin-log-test-test-instance.log");
    let contents = std::fs::read_to_string(&log_path)
      .unwrap_or_else(|err| panic!("Expected a log file at {:?} - {}", log_path, err));
    assert_eq!(contents, "hello\tworld\t42\na logger message\n");
  }

  #[tokio::test]
  async fn loads_the_jwt_plugin_and_runs_the_init_function() {
    let manifest = jwt_manifest();
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().unwrap();
    let entries = call_init(&lua, "test", "0.0.0").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "jwt");
    assert_eq!(entries[0].r#type, catalogue_entry::EntryType::ContentMatcher as i32);
    assert_eq!(entries[1].r#type, catalogue_entry::EntryType::ContentGenerator as i32);
  }

  #[tokio::test]
  async fn configure_interaction_then_match_contents_round_trip() {
    let manifest = jwt_manifest();
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let mut config_fields = HashMap::new();
    config_fields.insert("private-key".to_string(), serde_json::Value::String(PRIVATE_KEY.to_string()));
    config_fields.insert("subject".to_string(), serde_json::Value::String("test-subject".to_string()));
    config_fields.insert("issuer".to_string(), serde_json::Value::String("test-issuer".to_string()));
    config_fields.insert("audience".to_string(), serde_json::Value::String("test-audience".to_string()));
    config_fields.insert("algorithm".to_string(), serde_json::Value::String("RS512".to_string()));

    let configure_request = ConfigureInteractionRequest {
      content_type: "application/jwt+json".to_string(),
      contents_config: Some(to_proto_struct(&config_fields)),
    };
    let configure_response = plugin.configure_interaction(configure_request).await.unwrap();
    assert_eq!(configure_response.error, "");
    assert_eq!(configure_response.interaction.len(), 1);

    let interaction = &configure_response.interaction[0];
    let body = interaction.contents.clone().expect("expected a body");
    assert_eq!(body.content_type, "application/jwt+json");
    let token = String::from_utf8(body.content.clone().unwrap()).unwrap();
    assert_eq!(token.split('.').count(), 3);

    let compare_request = CompareContentsRequest {
      expected: Some(body.clone()),
      actual: Some(body),
      allow_unexpected_keys: false,
      rules: HashMap::new(),
      plugin_configuration: interaction.plugin_configuration.clone(),
    };
    let compare_response = plugin.compare_contents(compare_request).await.unwrap();
    assert_eq!(compare_response.error, "");
    assert!(compare_response.type_mismatch.is_none());
    assert!(
      compare_response.results.is_empty(),
      "expected no mismatches, got {:?}",
      compare_response.results
    );
  }

  #[tokio::test]
  async fn match_contents_detects_a_tampered_token() {
    let manifest = jwt_manifest();
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let mut config_fields = HashMap::new();
    config_fields.insert("private-key".to_string(), serde_json::Value::String(PRIVATE_KEY.to_string()));
    config_fields.insert("algorithm".to_string(), serde_json::Value::String("RS512".to_string()));

    let configure_request = ConfigureInteractionRequest {
      content_type: "application/jwt+json".to_string(),
      contents_config: Some(to_proto_struct(&config_fields)),
    };
    let configure_response = plugin.configure_interaction(configure_request).await.unwrap();
    let interaction = &configure_response.interaction[0];
    let expected_body = interaction.contents.clone().unwrap();

    let mut actual_body = expected_body.clone();
    let mut token = String::from_utf8(actual_body.content.clone().unwrap()).unwrap();
    token.push('x'); // tamper with the signature
    actual_body.content = Some(token.into_bytes());

    let compare_request = CompareContentsRequest {
      expected: Some(expected_body),
      actual: Some(actual_body),
      allow_unexpected_keys: false,
      rules: HashMap::new(),
      plugin_configuration: interaction.plugin_configuration.clone(),
    };
    let compare_response = plugin.compare_contents(compare_request).await.unwrap();
    assert!(!compare_response.results.is_empty(), "expected a mismatch to be detected");
  }

  #[tokio::test]
  async fn compare_contents_handles_non_string_mismatch_values() {
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        function match_contents(request)
          return {
            mismatches = {
              ["claims:exp"] = { expected = 123, actual = 456, mismatch = "exp differs", path = "claims:exp" },
              ["claims:verified"] = { expected = true, actual = false, mismatch = "verified differs", path = "claims:verified" }
            }
          }
        end
      "#,
    ).unwrap();

    let manifest = PactPluginManifest {
      plugin_dir: plugin_dir.path().to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: "scalar-mismatch-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    };
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let compare_request = CompareContentsRequest {
      expected: None,
      actual: None,
      allow_unexpected_keys: false,
      rules: HashMap::new(),
      plugin_configuration: None,
    };
    let response = plugin.compare_contents(compare_request).await.unwrap();

    let exp_mismatch = &response.results["claims:exp"].mismatches[0];
    assert_eq!(exp_mismatch.expected.as_deref(), Some("123".as_bytes()));
    assert_eq!(exp_mismatch.actual.as_deref(), Some("456".as_bytes()));

    let verified_mismatch = &response.results["claims:verified"].mismatches[0];
    assert_eq!(verified_mismatch.expected.as_deref(), Some("true".as_bytes()));
    assert_eq!(verified_mismatch.actual.as_deref(), Some("false".as_bytes()));
  }

  fn transport_manifest(plugin_dir: &std::path::Path, plugin_interface_version: u8) -> PactPluginManifest {
    PactPluginManifest {
      plugin_dir: plugin_dir.to_string_lossy().to_string(),
      plugin_interface_version,
      name: "transport-test".to_string(),
      version: "0.0.0".to_string(),
      executable_type: "lua".to_string(),
      minimum_required_version: None,
      entry_point: "entry.lua".to_string(),
      entry_points: HashMap::new(),
      args: None,
      dependencies: None,
      plugin_config: HashMap::new(),
    }
  }

  const TRANSPORT_PLUGIN_SCRIPT: &str = r#"
    function start_mock_server(request)
      START_MOCK_SERVER_REQUEST = request
      if request.port == 0 then
        return { error = "could not bind a mock server" }
      end
      return { details = { key = "mock-server-1", port = 12345, address = "127.0.0.1:12345" } }
    end

    function shutdown_mock_server(server_key)
      SHUTDOWN_SERVER_KEY = server_key
      return {
        ok = false,
        results = { { path = "/foo", error = "did not match", mismatches = { "simple string mismatch" } } }
      }
    end

    function get_mock_server_results(server_key)
      GET_RESULTS_SERVER_KEY = server_key
      return { ok = true, results = {} }
    end

    function prepare_interaction_for_verification(request)
      PREPARE_REQUEST = request
      return {
        interaction_data = {
          body = { content_type = "application/json", contents = "prepared-body", content_type_hint = "TEXT" },
          metadata = { path = "/foo", tag = { binary = "raw-bytes" } }
        }
      }
    end

    function verify_interaction(request)
      VERIFY_REQUEST = request
      if request.config ~= nil and request.config.fail == true then
        return { error = "verification failed" }
      end
      return {
        result = {
          success = true,
          response_data = { body = { content_type = "application/json", contents = "response-body" }, metadata = {} },
          mismatches = { "a plain mismatch", { mismatch = "a table mismatch", path = "$.foo", expected = 1, actual = 2 } },
          output = { "POST /foo", "200 OK" }
        }
      }
    end
  "#;

  fn start_transport_plugin(plugin_interface_version: u8) -> LuaPactPlugin {
    let plugin_dir = tempdir::TempDir::new("lua-transport-plugin-test").unwrap();
    std::fs::write(plugin_dir.path().join("entry.lua"), TRANSPORT_PLUGIN_SCRIPT).unwrap();
    let manifest = transport_manifest(plugin_dir.path(), plugin_interface_version);
    // The script is fully read into the Lua VM by `start_lua_plugin`, so the tempdir doesn't
    // need to outlive this call.
    start_lua_plugin(&manifest, "test-instance".to_string()).unwrap()
  }

  #[tokio::test]
  async fn start_mock_server_v1_round_trip() {
    let plugin = start_transport_plugin(1);
    let response = plugin.start_mock_server(StartMockServerRequest {
      host_interface: "127.0.0.1".to_string(),
      port: 8080,
      tls: false,
      pact: "{\"consumer\":{}}".to_string(),
      test_context: None,
    }).await.unwrap();
    match response.response.unwrap() {
      start_mock_server_response::Response::Details(details) => {
        assert_eq!(details.key, "mock-server-1");
        assert_eq!(details.port, 12345);
        assert_eq!(details.address, "127.0.0.1:12345");
      }
      other => panic!("expected mock server details, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn start_mock_server_v1_returns_the_lua_error() {
    let plugin = start_transport_plugin(1);
    let response = plugin.start_mock_server(StartMockServerRequest {
      host_interface: "127.0.0.1".to_string(),
      port: 0,
      tls: false,
      pact: "{}".to_string(),
      test_context: None,
    }).await.unwrap();
    match response.response.unwrap() {
      start_mock_server_response::Response::Error(err) => assert_eq!(err, "could not bind a mock server"),
      other => panic!("expected an error response, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn start_mock_server_v2_passes_structured_interactions() {
    let plugin = start_transport_plugin(2);
    let request = proto_v2::StartMockServerRequest {
      host_interface: "127.0.0.1".to_string(),
      port: 8080,
      tls: false,
      interactions: vec![proto_v2::InteractionContents {
        interaction_type: "Synchronous/HTTP".to_string(),
        plugin_configuration: None,
        consumer: "test-consumer".to_string(),
        provider: "test-provider".to_string(),
      }],
      test_context: None,
    };
    let response = plugin.start_mock_server_v2(request).await.unwrap();
    assert!(matches!(response.response.unwrap(), start_mock_server_response::Response::Details(_)));

    let lua = plugin.runtime.lock().unwrap();
    let captured: Table = lua.globals().get("START_MOCK_SERVER_REQUEST").unwrap();
    let interactions: Table = captured.get("interactions").unwrap();
    let first: Table = interactions.get(1).unwrap();
    assert_eq!(first.get::<String>("interaction_type").unwrap(), "Synchronous/HTTP");
    assert_eq!(first.get::<String>("consumer").unwrap(), "test-consumer");
  }

  #[tokio::test]
  async fn shutdown_and_get_mock_server_results_parse_mismatches() {
    let plugin = start_transport_plugin(1);

    let shutdown_response = plugin.shutdown_mock_server(ShutdownMockServerRequest {
      server_key: "mock-server-1".to_string(),
    }).await.unwrap();
    assert!(!shutdown_response.ok);
    assert_eq!(shutdown_response.results.len(), 1);
    assert_eq!(shutdown_response.results[0].path, "/foo");
    assert_eq!(shutdown_response.results[0].mismatches[0].mismatch, "simple string mismatch");

    let results_response = plugin.get_mock_server_results(MockServerRequest {
      server_key: "mock-server-1".to_string(),
    }).await.unwrap();
    assert!(results_response.ok);
    assert!(results_response.results.is_empty());
  }

  #[tokio::test]
  async fn prepare_interaction_for_verification_v1_round_trip() {
    let plugin = start_transport_plugin(1);
    let response = plugin.prepare_interaction_for_verification(VerificationPreparationRequest {
      pact: "{}".to_string(),
      interaction_key: "interaction-1".to_string(),
      config: None,
    }).await.unwrap();

    match response.response.unwrap() {
      verification_preparation_response::Response::InteractionData(data) => {
        let body = data.body.unwrap();
        assert_eq!(body.content, Some("prepared-body".as_bytes().to_vec()));
        let metadata = data.metadata;
        assert!(matches!(
          metadata["path"].value,
          Some(metadata_value::Value::NonBinaryValue(_))
        ));
        match &metadata["tag"].value {
          Some(metadata_value::Value::BinaryValue(bytes)) => assert_eq!(bytes, b"raw-bytes"),
          other => panic!("expected a binary metadata value, got {:?}", other),
        }
      }
      other => panic!("expected interaction data, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn prepare_interaction_for_verification_v2_passes_interaction_contents() {
    let plugin = start_transport_plugin(2);
    let request = proto_v2::VerificationPreparationRequest {
      interaction_contents: Some(proto_v2::InteractionContents {
        interaction_type: "Synchronous/HTTP".to_string(),
        plugin_configuration: None,
        consumer: "test-consumer".to_string(),
        provider: "test-provider".to_string(),
      }),
      config: None,
      test_context: None,
    };
    let response = plugin.prepare_interaction_for_verification_v2(request).await.unwrap();
    assert!(matches!(
      response.response.unwrap(),
      verification_preparation_response::Response::InteractionData(_)
    ));

    let lua = plugin.runtime.lock().unwrap();
    let captured: Table = lua.globals().get("PREPARE_REQUEST").unwrap();
    let interaction_contents: Table = captured.get("interaction_contents").unwrap();
    assert_eq!(interaction_contents.get::<String>("provider").unwrap(), "test-provider");
  }

  #[tokio::test]
  async fn verify_interaction_v1_round_trip() {
    let plugin = start_transport_plugin(1);
    let mut metadata = HashMap::new();
    metadata.insert("path".to_string(), MetadataValue {
      value: Some(metadata_value::Value::NonBinaryValue(prost_types::Value {
        kind: Some(prost_types::value::Kind::StringValue("/foo".to_string())),
      })),
    });
    let response = plugin.verify_interaction(VerifyInteractionRequest {
      interaction_data: Some(InteractionData {
        body: Some(Body {
          content_type: "application/json".to_string(),
          content: Some("request-body".as_bytes().to_vec()),
          content_type_hint: body::ContentTypeHint::Text as i32,
        }),
        metadata,
      }),
      config: None,
      pact: "{}".to_string(),
      interaction_key: "interaction-1".to_string(),
    }).await.unwrap();

    match response.response.unwrap() {
      verify_interaction_response::Response::Result(result) => {
        assert!(result.success);
        assert_eq!(result.output, vec!["POST /foo".to_string(), "200 OK".to_string()]);
        assert_eq!(result.mismatches.len(), 2);
        match &result.mismatches[0].result {
          Some(verification_result_item::Result::Error(err)) => assert_eq!(err, "a plain mismatch"),
          other => panic!("expected an error mismatch, got {:?}", other),
        }
        match &result.mismatches[1].result {
          Some(verification_result_item::Result::Mismatch(mismatch)) => {
            assert_eq!(mismatch.mismatch, "a table mismatch");
            assert_eq!(mismatch.expected, Some(b"1".to_vec()));
          }
          other => panic!("expected a mismatch, got {:?}", other),
        }
      }
      other => panic!("expected a verification result, got {:?}", other),
    }

    let lua = plugin.runtime.lock().unwrap();
    let captured: Table = lua.globals().get("VERIFY_REQUEST").unwrap();
    let interaction_data: Table = captured.get("interaction_data").unwrap();
    let metadata: Table = interaction_data.get("metadata").unwrap();
    assert_eq!(metadata.get::<String>("path").unwrap(), "/foo");
  }

  #[tokio::test]
  async fn verify_interaction_v1_returns_the_lua_error() {
    let plugin = start_transport_plugin(1);
    let mut config = HashMap::new();
    config.insert("fail".to_string(), serde_json::Value::Bool(true));
    let response = plugin.verify_interaction(VerifyInteractionRequest {
      interaction_data: None,
      config: Some(to_proto_struct(&config)),
      pact: "{}".to_string(),
      interaction_key: "interaction-1".to_string(),
    }).await.unwrap();
    match response.response.unwrap() {
      verify_interaction_response::Response::Error(err) => assert_eq!(err, "verification failed"),
      other => panic!("expected an error response, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn verify_interaction_v2_converts_the_v2_interaction_data_and_contents() {
    let plugin = start_transport_plugin(2);
    let request = proto_v2::VerifyInteractionRequest {
      interaction_data: Some(proto_v2::InteractionData {
        body: Some(proto_v2::Body {
          content_type: "application/json".to_string(),
          content: Some("request-body".as_bytes().to_vec()),
          content_type_hint: 0,
        }),
        metadata: HashMap::new(),
      }),
      config: None,
      interaction_contents: Some(proto_v2::InteractionContents {
        interaction_type: "Synchronous/HTTP".to_string(),
        plugin_configuration: None,
        consumer: "test-consumer".to_string(),
        provider: "test-provider".to_string(),
      }),
      test_context: None,
    };
    let response = plugin.verify_interaction_v2(request).await.unwrap();
    assert!(matches!(
      response.response.unwrap(),
      verify_interaction_response::Response::Result(_)
    ));

    let lua = plugin.runtime.lock().unwrap();
    let captured: Table = lua.globals().get("VERIFY_REQUEST").unwrap();
    let interaction_data: Table = captured.get("interaction_data").unwrap();
    let body: Table = interaction_data.get("body").unwrap();
    assert_eq!(body.get::<mlua::String>("contents").unwrap().to_str().unwrap(), "request-body");
    let interaction_contents: Table = captured.get("interaction_contents").unwrap();
    assert_eq!(interaction_contents.get::<String>("consumer").unwrap(), "test-consumer");
  }

  #[tokio::test]
  async fn shutdown_mock_server_defaults_ok_to_true_when_the_field_is_absent() {
    // Regression test: `Table::get::<bool>("ok")` converts a *missing* key's Lua nil straight
    // to `false` (mlua's bool conversion, matching Lua's own nil-is-falsy semantics) rather than
    // erroring - so a plain `.unwrap_or(true)` fallback was never reached, and an `ok`-less
    // response used to silently report `ok = false` instead of the documented default of
    // `true`. Reading as `Option<bool>` first lets a missing key correctly fall through to the
    // `unwrap_or(true)` default, since `Option<T>` intercepts Lua nil before the inner
    // conversion happens.
    let plugin_dir = tempdir::TempDir::new("lua-transport-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        function shutdown_mock_server(server_key)
          return { results = {} }
        end
      "#,
    ).unwrap();
    let manifest = transport_manifest(plugin_dir.path(), 1);
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let response = plugin.shutdown_mock_server(ShutdownMockServerRequest {
      server_key: "mock-server-1".to_string(),
    }).await.unwrap();
    assert!(response.ok, "expected 'ok' to default to true when the Lua script doesn't set it");
  }

  #[tokio::test]
  async fn shutdown_mock_server_errors_on_a_wrong_typed_path_field() {
    let plugin_dir = tempdir::TempDir::new("lua-transport-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        function shutdown_mock_server(server_key)
          return { ok = false, results = { { path = {}, error = "boom", mismatches = {} } } }
        end
      "#,
    ).unwrap();
    let manifest = transport_manifest(plugin_dir.path(), 1);
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let result = plugin.shutdown_mock_server(ShutdownMockServerRequest {
      server_key: "mock-server-1".to_string(),
    }).await;
    assert!(
      result.is_err(),
      "expected a wrong-typed 'path' field (a table, not a string) to be a hard error, not silently default, got {:?}",
      result
    );
  }
}
