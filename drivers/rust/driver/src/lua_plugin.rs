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
use crate::utils::{proto_struct_to_json, to_proto_struct};

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
  set_package_path(&lua, &script_path)?;
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

fn set_package_path(lua: &Lua, script_path: &Path) -> anyhow::Result<()> {
  let script_dir = script_path.parent().unwrap_or_else(|| Path::new("."));
  let package: Table = lua.globals().get("package")?;
  let existing: String = package.get("path").unwrap_or_default();
  let new_path = format!("{}/?.lua;{}", script_dir.to_string_lossy(), existing);
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
    _request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    Err(anyhow!("Mock servers are not supported by Lua content-matcher plugins"))
  }

  async fn start_mock_server_v2(
    &self,
    _request: proto_v2::StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    Err(anyhow!("Mock servers are not supported by Lua content-matcher plugins"))
  }

  async fn shutdown_mock_server(
    &self,
    _request: ShutdownMockServerRequest,
  ) -> anyhow::Result<ShutdownMockServerResponse> {
    Err(anyhow!("Mock servers are not supported by Lua content-matcher plugins"))
  }

  async fn get_mock_server_results(
    &self,
    _request: MockServerRequest,
  ) -> anyhow::Result<MockServerResults> {
    Err(anyhow!("Mock servers are not supported by Lua content-matcher plugins"))
  }

  async fn prepare_interaction_for_verification(
    &self,
    _request: VerificationPreparationRequest,
  ) -> anyhow::Result<VerificationPreparationResponse> {
    Err(anyhow!(
      "prepare_interaction_for_verification is only supported by TRANSPORT plugins, not Lua content-matcher plugins"
    ))
  }

  async fn verify_interaction(
    &self,
    _request: VerifyInteractionRequest,
  ) -> anyhow::Result<VerifyInteractionResponse> {
    Err(anyhow!(
      "verify_interaction is only supported by TRANSPORT plugins, not Lua content-matcher plugins"
    ))
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
    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("../../../plugins/jwt")
      .canonicalize()
      .expect("plugins/jwt directory should exist");
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
}
