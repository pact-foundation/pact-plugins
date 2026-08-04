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
//!
//! From within `match_contents`/`generate_content`, a script can also call back into a
//! host-provided or another plugin's capability, named by catalogue entry key (proposal 007,
//! "Lua transport" - the in-process equivalent of the gRPC `PluginHost` callback service):
//!
//! - `host_compare_contents(entry_key, request) -> table` - same request/response shape as
//!   `match_contents` itself, so a result can be returned straight through.
//! - `host_generate_content(entry_key, contents, generators, test_mode) -> body` - same
//!   arguments and return shape as `generate_content` itself.
//!
//! These are only reachable from Lua code running inside `match_contents`/`generate_content`
//! (the two entry points the driver invokes via `call_async`); calling them from another entry
//! point currently fails, since mlua can only resolve an async host function from within a Lua
//! call chain that was itself started asynchronously.

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
use tracing::{debug, warn};

use crate::call_chain;
use crate::catalogue_manager::{CatalogueEntryType, ResolvedCapability, resolve_capability};
use crate::plugin_manager::lookup_plugin;
use crate::plugin_models::{
  PactPluginManifest, PactPluginRpc, PluginInitRequest, PluginInitResponse, PluginInstance,
};
use crate::proto::*;
use crate::proto_v2;
use crate::utils::{proto_struct_to_json, proto_value_to_json, to_proto_struct, to_proto_value};

/// A running Lua plugin instance. Each instance owns its own embedded Lua VM.
///
/// The mutex is `tokio::sync::Mutex`, not `std::sync::Mutex`: the two host functions plugins can
/// call to reach a host-provided or another plugin's capability (`host_compare_contents`,
/// `host_generate_content` - see [`register_host_functions`] and proposal 007) need to hold the
/// lock across an `.await` while they dispatch to an async [`crate::core_capabilities`] handler
/// or forward to another plugin, which a `std::sync::MutexGuard` cannot do (it isn't `Send`,
/// and `PluginInstance`'s `#[async_trait]` methods require a `Send` future).
pub struct LuaPactPlugin {
  runtime: Arc<tokio::sync::Mutex<Lua>>,
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
    runtime: Arc::new(tokio::sync::Mutex::new(lua)),
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
    lua.create_function(|_, (data, key): (mlua::LuaString, String)| {
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

  // Callback host functions (proposal 007, "Lua transport"): let a plugin script delegate to a
  // host-provided or another plugin's content matcher/generator, named by catalogue entry key,
  // instead of reimplementing it. Registered as *async* Lua functions - resolving the entry may
  // need to await an async core capability handler or forward the call to another plugin - which
  // is why the script's own `match_contents`/`generate_content` are invoked via `call_async` (see
  // `PluginInstance::compare_contents`/`generate_content` below): mlua only allows an async host
  // function to be reached from a Lua call chain that was itself started with `call_async`.
  //
  // No call-chain ID or cycle detection is needed here, unlike the gRPC `PluginHost` callback
  // path (`plugin_host.rs`) - this is a direct, synchronous (from Lua's perspective) Rust call;
  // a true cycle shows up as a native stack overflow, the same reasoning that applies to WASM.
  globals.set(
    "host_compare_contents",
    lua.create_async_function(move |lua, (entry_key, request): (String, Table)| async move {
      let request = lua_to_compare_request(&lua, request).map_err(mlua::Error::external)?;
      let response = call_host_compare_contents(&entry_key, request).await.map_err(mlua::Error::external)?;
      compare_response_to_lua(&lua, &response)
    })?,
  )?;

  globals.set(
    "host_generate_content",
    lua.create_async_function(move |lua, (entry_key, contents, generators, test_mode): (String, Value, Option<Table>, Option<String>)| async move {
      let request = lua_to_generate_request(&lua, contents, generators, test_mode).map_err(mlua::Error::external)?;
      let response = call_host_generate_content(&entry_key, request).await.map_err(mlua::Error::external)?;
      body_to_lua(&lua, &response.contents)
    })?,
  )?;

  // The field-level equivalents (proposal 006, "Lua transport"): let a plugin that owns a content
  // type delegate one value inside it to a standard Pact rule - or to another plugin's rule -
  // instead of reimplementing it. Async and cycle-free for the same reasons as the two above.
  globals.set(
    "host_match_field",
    lua.create_async_function(move |lua, (entry_key, request): (String, Table)| async move {
      let request = lua_to_match_field_request(&lua, request).map_err(mlua::Error::external)?;
      let response = call_host_match_field(&entry_key, request).await.map_err(mlua::Error::external)?;
      match_field_response_to_lua(&lua, &response)
    })?,
  )?;

  globals.set(
    "host_generate_field",
    lua.create_async_function(move |lua, (entry_key, request): (String, Table)| async move {
      let request = lua_to_generate_field_request(&lua, request).map_err(mlua::Error::external)?;
      let response = call_host_generate_field(&entry_key, request).await.map_err(mlua::Error::external)?;
      generate_field_response_to_lua(&lua, &response)
    })?,
  )?;

  Ok(())
}

/// Resolve `entry_key` to a content matcher capability and dispatch to it - a host-registered
/// [`crate::core_capabilities::CoreContentMatcher`] called in-process, or another running plugin
/// called via a freshly-started call chain (see [`crate::call_chain`]), matching the same
/// resolver [`crate::plugin_host`] uses for the gRPC callback path. Backs the `host_compare_contents`
/// Lua host function.
async fn call_host_compare_contents(
  entry_key: &str,
  request: CompareContentsRequest
) -> anyhow::Result<CompareContentsResponse> {
  match resolve_capability(entry_key, CatalogueEntryType::CONTENT_MATCHER)? {
    ResolvedCapability::Core(core_key) => {
      let handler = crate::core_capabilities::lookup_core_content_matcher(&core_key)
        .ok_or_else(|| anyhow!("No core content matcher registered for '{}'", core_key))?;
      handler.compare_contents(request).await
    }
    ResolvedCapability::Plugin(manifest) => {
      let plugin = lookup_plugin(&manifest.as_dependency())
        .ok_or_else(|| anyhow!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key))?;
      let chain_id = call_chain::new_call_chain_id();
      let deadline_ms = call_chain::default_deadline_ms();
      plugin.compare_contents_with_chain(request, &chain_id, deadline_ms).await
    }
  }
}

/// Resolve `entry_key` to a content generator capability and dispatch to it. See
/// [`call_host_compare_contents`]; backs the `host_generate_content` Lua host function.
async fn call_host_generate_content(
  entry_key: &str,
  request: GenerateContentRequest
) -> anyhow::Result<GenerateContentResponse> {
  match resolve_capability(entry_key, CatalogueEntryType::CONTENT_GENERATOR)? {
    ResolvedCapability::Core(core_key) => {
      let handler = crate::core_capabilities::lookup_core_content_generator(&core_key)
        .ok_or_else(|| anyhow!("No core content generator registered for '{}'", core_key))?;
      handler.generate_content(request).await
    }
    ResolvedCapability::Plugin(manifest) => {
      let plugin = lookup_plugin(&manifest.as_dependency())
        .ok_or_else(|| anyhow!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key))?;
      let chain_id = call_chain::new_call_chain_id();
      let deadline_ms = call_chain::default_deadline_ms();
      plugin.generate_content_with_chain(request, &chain_id, deadline_ms).await
    }
  }
}

/// Resolve `entry_key` to a field-level matching rule and dispatch to it. See
/// [`call_host_compare_contents`]; backs the `host_match_field` Lua host function.
async fn call_host_match_field(
  entry_key: &str,
  request: proto_v2::MatchFieldRequest
) -> anyhow::Result<proto_v2::MatchFieldResponse> {
  match resolve_capability(entry_key, CatalogueEntryType::MATCHER)? {
    ResolvedCapability::Core(core_key) => {
      let handler = crate::core_capabilities::lookup_core_field_matcher(&core_key)
        .ok_or_else(|| anyhow!("No core field matcher registered for '{}'", core_key))?;
      handler.match_field(request).await
    }
    ResolvedCapability::Plugin(manifest) => {
      let plugin = lookup_plugin(&manifest.as_dependency())
        .ok_or_else(|| anyhow!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key))?;
      let chain_id = call_chain::new_call_chain_id();
      let deadline_ms = call_chain::default_deadline_ms();
      plugin.match_field_with_chain(request, &chain_id, deadline_ms).await
    }
  }
}

/// Resolve `entry_key` to a field-level generator and dispatch to it. See
/// [`call_host_compare_contents`]; backs the `host_generate_field` Lua host function.
async fn call_host_generate_field(
  entry_key: &str,
  request: proto_v2::GenerateFieldRequest
) -> anyhow::Result<proto_v2::GenerateFieldResponse> {
  match resolve_capability(entry_key, CatalogueEntryType::GENERATOR)? {
    ResolvedCapability::Core(core_key) => {
      let handler = crate::core_capabilities::lookup_core_field_generator(&core_key)
        .ok_or_else(|| anyhow!("No core field generator registered for '{}'", core_key))?;
      handler.generate_field(request).await
    }
    ResolvedCapability::Plugin(manifest) => {
      let plugin = lookup_plugin(&manifest.as_dependency())
        .ok_or_else(|| anyhow!("Plugin '{}' for entry '{}' is not currently running", manifest.name, entry_key))?;
      let chain_id = call_chain::new_call_chain_id();
      let deadline_ms = call_chain::default_deadline_ms();
      plugin.generate_field_with_chain(request, &chain_id, deadline_ms).await
    }
  }
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
    let entry_type = CatalogueEntryType::from_proto_name(&entry_type_str)
      .ok_or_else(|| anyhow!("Unknown catalogue entry type '{}'", entry_type_str))?;
    entries.push(CatalogueEntry {
      r#type: entry_type.to_proto_value(),
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
      let contents: Option<mlua::LuaString> = table.get("contents")?;
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

/// Reverse of [`matching_rules_to_lua`] - used by `host_compare_contents` to convert the rules a
/// plugin script builds when calling back into a host-provided or another plugin's matcher.
fn lua_to_matching_rules(lua: &Lua, table: Option<Table>) -> anyhow::Result<HashMap<String, MatchingRules>> {
  let mut result = HashMap::new();
  if let Some(table) = table {
    for pair in table.pairs::<String, Table>() {
      let (path, rules_table) = pair?;
      let mut rule = vec![];
      for rule_value in rules_table.sequence_values::<Table>() {
        let rule_table = rule_value?;
        let r#type: String = rule_table.get("type")?;
        let values: Option<Value> = rule_table.get("values")?;
        let values = match values {
          Some(value) => Some(to_proto_struct(&as_json_map(lua.from_value(value)?))),
          None => None,
        };
        rule.push(MatchingRule { r#type, values });
      }
      result.insert(path, MatchingRules { rule });
    }
  }
  Ok(result)
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

/// Reverse of [`compare_request_to_lua`] - the request table shape a plugin script builds when
/// calling `host_compare_contents(entry_key, request)` (see [`register_host_functions`]) is the
/// same shape its own `match_contents(request)` function receives.
fn lua_to_compare_request(lua: &Lua, table: Table) -> anyhow::Result<CompareContentsRequest> {
  let expected: Value = table.get("expected")?;
  let actual: Value = table.get("actual")?;
  let allow_unexpected_keys: Option<bool> = table.get("allow_unexpected_keys")?;
  let rules: Option<Table> = table.get("rules")?;
  let plugin_configuration: Option<Value> = table.get("plugin_configuration")?;
  Ok(CompareContentsRequest {
    expected: lua_to_body(expected)?,
    actual: lua_to_body(actual)?,
    allow_unexpected_keys: allow_unexpected_keys.unwrap_or(false),
    rules: lua_to_matching_rules(lua, rules)?,
    plugin_configuration: lua_to_plugin_configuration(lua, plugin_configuration)?,
  })
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

/// Converts a path's mismatches into the sequence-of-tables shape
/// [`lua_value_to_content_mismatches`] parses, so a `host_compare_contents` response can be
/// passed straight through as part of the calling plugin's own `match_contents` response.
fn content_mismatches_to_lua(lua: &Lua, mismatches: &[ContentMismatch]) -> mlua::Result<Table> {
  let list = lua.create_table()?;
  for mismatch in mismatches {
    let table = lua.create_table()?;
    table.set("mismatch", mismatch.mismatch.clone())?;
    if let Some(expected) = &mismatch.expected {
      table.set("expected", lua.create_string(expected)?)?;
    }
    if let Some(actual) = &mismatch.actual {
      table.set("actual", lua.create_string(actual)?)?;
    }
    table.set("path", mismatch.path.clone())?;
    if !mismatch.diff.is_empty() {
      table.set("diff", mismatch.diff.clone())?;
    }
    if !mismatch.mismatch_type.is_empty() {
      table.set("mismatch_type", mismatch.mismatch_type.clone())?;
    }
    list.push(table)?;
  }
  Ok(list)
}

/// Reverse of [`lua_to_compare_response`] - the table `host_compare_contents` returns is shaped
/// exactly like what a plugin's own `match_contents` function is expected to return, so a plugin
/// can pass a host/forwarded comparison's result straight through as its own response.
fn compare_response_to_lua(lua: &Lua, response: &CompareContentsResponse) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  if !response.error.is_empty() {
    table.set("error", response.error.clone())?;
    return Ok(table);
  }
  if let Some(type_mismatch) = &response.type_mismatch {
    let mismatch_table = lua.create_table()?;
    mismatch_table.set("expected", type_mismatch.expected.clone())?;
    mismatch_table.set("actual", type_mismatch.actual.clone())?;
    table.set("type-mismatch", mismatch_table)?;
    return Ok(table);
  }
  if !response.results.is_empty() {
    let mismatches_table = lua.create_table()?;
    for (path, content_mismatches) in &response.results {
      mismatches_table.set(path.clone(), content_mismatches_to_lua(lua, &content_mismatches.mismatches)?)?;
    }
    table.set("mismatches", mismatches_table)?;
  }
  Ok(table)
}

// ---- GenerateContent <-> Lua ----

/// Converts the `(entry_key, contents, generators, test_mode)` arguments a plugin script passes
/// to `host_generate_content` (see [`register_host_functions`]) into a `GenerateContentRequest` -
/// the same three trailing arguments its own `generate_content(contents, generators, test_mode)`
/// function receives.
fn lua_to_generate_request(
  lua: &Lua,
  contents: Value,
  generators: Option<Table>,
  test_mode: Option<String>
) -> anyhow::Result<GenerateContentRequest> {
  let mut generator_map = HashMap::new();
  if let Some(generators) = generators {
    for pair in generators.pairs::<String, Table>() {
      let (path, generator_table) = pair?;
      let r#type: String = generator_table.get("type")?;
      let values: Option<Value> = generator_table.get("values")?;
      let values = match values {
        Some(value) => Some(to_proto_struct(&as_json_map(lua.from_value(value)?))),
        None => None,
      };
      generator_map.insert(path, Generator { r#type, values });
    }
  }
  Ok(GenerateContentRequest {
    contents: lua_to_body(contents)?,
    generators: generator_map,
    test_mode: str_to_test_mode(test_mode.as_deref()),
    .. GenerateContentRequest::default()
  })
}

/// The test mode name a Lua script sees. The V1 and V2 `GenerateContentRequest.TestMode` enums
/// have the same values, so one pair of helpers covers content generation on either interface as
/// well as field-level generation (which is V2-only).
fn test_mode_to_str(test_mode: i32) -> &'static str {
  match generate_content_request::TestMode::try_from(test_mode)
    .unwrap_or(generate_content_request::TestMode::Unknown)
  {
    generate_content_request::TestMode::Consumer => "Consumer",
    generate_content_request::TestMode::Provider => "Provider",
    generate_content_request::TestMode::Unknown => "Unknown",
  }
}

/// Reverse of [`test_mode_to_str`]. Anything unrecognised (including a missing value) is
/// `Unknown`, rather than an error - the mode is context for the plugin, not a contract.
fn str_to_test_mode(test_mode: Option<&str>) -> i32 {
  match test_mode {
    Some("Consumer") => generate_content_request::TestMode::Consumer as i32,
    Some("Provider") => generate_content_request::TestMode::Provider as i32,
    _ => generate_content_request::TestMode::Unknown as i32,
  }
}

// ---- MatchField / GenerateField <-> Lua ----

/// Converts a single field value to a plain Lua value, following the convention message metadata
/// values already use (see [`metadata_to_lua`]): everything crosses as a plain Lua value except
/// binary data, which arrives as a `{ binary = <lua string> }` wrapper table so a script can tell
/// a string of text from a blob of bytes.
///
/// Lua 5.4 has separate integer and float subtypes, so a whole number stays whole across the
/// boundary. That distinction is what the `integer`, `decimal` and `type` rules are built on, and
/// is the reason `FieldValue` has an arm per scalar type in the first place - see proposal 006,
/// section 5.
fn field_value_to_lua(lua: &Lua, value: &Option<proto_v2::FieldValue>) -> mlua::Result<Value> {
  use proto_v2::field_value::Value as FieldValue;
  match value.as_ref().and_then(|value| value.value.as_ref()) {
    None | Some(FieldValue::NullValue(_)) => Ok(Value::Nil),
    Some(FieldValue::BooleanValue(value)) => Ok(Value::Boolean(*value)),
    Some(FieldValue::StringValue(value)) => Ok(Value::String(lua.create_string(value)?)),
    Some(FieldValue::IntegerValue(value)) => Ok(Value::Integer(*value)),
    Some(FieldValue::DecimalValue(value)) => Ok(Value::Number(*value)),
    Some(FieldValue::BinaryValue(bytes)) => {
      let wrapper = lua.create_table()?;
      wrapper.set("binary", lua.create_string(bytes)?)?;
      Ok(Value::Table(wrapper))
    }
    Some(FieldValue::StructuredValue(value)) => lua.to_value(&proto_value_to_json(value)),
  }
}

/// Reverse of [`field_value_to_lua`]. A table is a binary wrapper if it has a `binary` key, and a
/// map or list otherwise - the same test [`lua_to_metadata`] applies.
fn lua_to_field_value(lua: &Lua, value: Value) -> anyhow::Result<proto_v2::FieldValue> {
  use proto_v2::field_value::Value as FieldValue;
  let value = match value {
    Value::Nil => FieldValue::NullValue(0),
    Value::Boolean(value) => FieldValue::BooleanValue(value),
    Value::Integer(value) => FieldValue::IntegerValue(value),
    Value::Number(value) => FieldValue::DecimalValue(value),
    Value::String(value) => FieldValue::StringValue(value.to_str()?.to_string()),
    Value::Table(table) => match table.get::<Option<mlua::String>>("binary")? {
      Some(binary) => FieldValue::BinaryValue(binary.as_bytes().to_vec()),
      None => FieldValue::StructuredValue(to_proto_value(&lua.from_value(Value::Table(table))?)),
    },
    other => return Err(anyhow!("Expected a field value from Lua, got {}", other.type_name())),
  };
  Ok(proto_v2::FieldValue { value: Some(value) })
}

/// Converts a matching rule or a generator - each is a name plus an optional struct of configured
/// values - into the `{ type = "...", values = { ... } }` table a script already sees for the
/// rules and generators passed to `match_contents`/`generate_content`. Always a table, even with
/// no values, so a script can read `rule.values` without checking `rule` first.
fn typed_values_to_lua(
  lua: &Lua,
  r#type: &str,
  values: &Option<prost_types::Struct>
) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  table.set("type", r#type.to_string())?;
  if let Some(values) = values {
    table.set("values", lua.to_value(&proto_struct_to_json(values))?)?;
  }
  Ok(table)
}

/// Reverse of [`typed_values_to_lua`], returning the name and values separately so the caller can
/// build either a `MatchingRule` or a `Generator` from them.
fn lua_to_typed_values(
  lua: &Lua,
  table: Option<Table>,
  what: &str
) -> anyhow::Result<(String, Option<prost_types::Struct>)> {
  let table = table.ok_or_else(|| anyhow!("Expected a '{}' table from Lua", what))?;
  let r#type: String = table.get("type")
    .map_err(|_| anyhow!("Expected the '{}' table from Lua to have a 'type'", what))?;
  let values: Option<Value> = table.get("values")?;
  let values = match values {
    Some(Value::Nil) | None => None,
    Some(value) => Some(to_proto_struct(&as_json_map(lua.from_value(value)?))),
  };
  Ok((r#type, values))
}

/// V2's `PluginConfiguration` carries the same two `Struct` fields as the V1 message the rest of
/// this module converts, so the field-level requests reuse those conversions rather than
/// duplicating them. The conversion is needed at all only because field-level operations exist
/// solely on the V2 interface (proposal 006).
fn v2_plugin_configuration_to_v1(
  config: &Option<proto_v2::PluginConfiguration>
) -> Option<PluginConfiguration> {
  config.as_ref().map(|config| PluginConfiguration {
    interaction_configuration: config.interaction_configuration.clone(),
    pact_configuration: config.pact_configuration.clone(),
  })
}

/// Reverse of [`v2_plugin_configuration_to_v1`].
fn v1_plugin_configuration_to_v2(
  config: Option<PluginConfiguration>
) -> Option<proto_v2::PluginConfiguration> {
  config.map(|config| proto_v2::PluginConfiguration {
    interaction_configuration: config.interaction_configuration,
    pact_configuration: config.pact_configuration,
  })
}

/// See [`v2_plugin_configuration_to_v1`] - `ContentMismatch` is likewise identical between the two
/// interfaces, so the existing mismatch conversions are reused for the V2-only field messages.
fn v1_content_mismatch_to_v2(mismatch: ContentMismatch) -> proto_v2::ContentMismatch {
  proto_v2::ContentMismatch {
    expected: mismatch.expected,
    actual: mismatch.actual,
    mismatch: mismatch.mismatch,
    path: mismatch.path,
    diff: mismatch.diff,
    mismatch_type: mismatch.mismatch_type,
  }
}

/// Reverse of [`v1_content_mismatch_to_v2`].
fn v2_content_mismatch_to_v1(mismatch: &proto_v2::ContentMismatch) -> ContentMismatch {
  ContentMismatch {
    expected: mismatch.expected.clone(),
    actual: mismatch.actual.clone(),
    mismatch: mismatch.mismatch.clone(),
    path: mismatch.path.clone(),
    diff: mismatch.diff.clone(),
    mismatch_type: mismatch.mismatch_type.clone(),
  }
}

/// Builds the request table a plugin's own `match_field(request)` function receives.
fn match_field_request_to_lua(lua: &Lua, request: &proto_v2::MatchFieldRequest) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  table.set("key", request.key.clone())?;
  let rule = request.rule.clone().unwrap_or_default();
  table.set("rule", typed_values_to_lua(lua, &rule.r#type, &rule.values)?)?;
  table.set("path", request.path.clone())?;
  table.set("mismatch_type", request.mismatch_type.clone())?;
  table.set("expected", field_value_to_lua(lua, &request.expected)?)?;
  table.set("actual", field_value_to_lua(lua, &request.actual)?)?;
  table.set(
    "plugin_configuration",
    plugin_configuration_to_lua(lua, &v2_plugin_configuration_to_v1(&request.plugin_configuration))?,
  )?;
  table.set("test_context", struct_to_lua(lua, &request.test_context)?)?;
  Ok(table)
}

/// Reverse of [`match_field_request_to_lua`] - the table a script builds when calling
/// `host_match_field(entry_key, request)` is the same shape its own `match_field` receives, so it
/// can forward the request it was given after adjusting whatever it needs to.
fn lua_to_match_field_request(lua: &Lua, table: Table) -> anyhow::Result<proto_v2::MatchFieldRequest> {
  let (r#type, values) = lua_to_typed_values(lua, table.get("rule")?, "rule")?;
  let plugin_configuration: Option<Value> = table.get("plugin_configuration")?;
  let test_context: Option<Value> = table.get("test_context")?;
  Ok(proto_v2::MatchFieldRequest {
    key: table.get::<Option<String>>("key")?.unwrap_or_default(),
    rule: Some(proto_v2::MatchingRule { r#type, values }),
    path: table.get::<Option<String>>("path")?.unwrap_or_default(),
    mismatch_type: table.get::<Option<String>>("mismatch_type")?.unwrap_or_default(),
    expected: Some(lua_to_field_value(lua, table.get("expected")?)?),
    actual: Some(lua_to_field_value(lua, table.get("actual")?)?),
    plugin_configuration: v1_plugin_configuration_to_v2(
      lua_to_plugin_configuration(lua, plugin_configuration)?
    ),
    test_context: lua_to_struct(lua, test_context)?,
  })
}

/// Parses the table a plugin's `match_field` function returns: `{ error = "..." }`, or
/// `{ mismatches = { ... } }` where each entry is a mismatch table or a bare description string
/// (the same leniency `match_contents` responses get - see [`lua_value_to_content_mismatches`]).
/// An absent or empty list means the value matched.
fn lua_to_match_field_response(table: Table, path: &str) -> anyhow::Result<proto_v2::MatchFieldResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(proto_v2::MatchFieldResponse { error, mismatches: vec![] });
  }

  let mismatches: Option<Value> = table.get("mismatches")?;
  let mismatches = match mismatches {
    Some(value) => lua_value_to_content_mismatches(path, value)?
      .into_iter()
      .map(v1_content_mismatch_to_v2)
      .collect(),
    None => vec![],
  };
  Ok(proto_v2::MatchFieldResponse { error: String::new(), mismatches })
}

/// Reverse of [`lua_to_match_field_response`], so a script can return the result of a
/// `host_match_field` call straight through as its own response.
fn match_field_response_to_lua(lua: &Lua, response: &proto_v2::MatchFieldResponse) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  if !response.error.is_empty() {
    table.set("error", response.error.clone())?;
    return Ok(table);
  }
  let mismatches: Vec<ContentMismatch> = response.mismatches.iter()
    .map(v2_content_mismatch_to_v1)
    .collect();
  table.set("mismatches", content_mismatches_to_lua(lua, &mismatches)?)?;
  Ok(table)
}

/// Builds the request table a plugin's own `generate_field(request)` function receives.
fn generate_field_request_to_lua(lua: &Lua, request: &proto_v2::GenerateFieldRequest) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  table.set("key", request.key.clone())?;
  let generator = request.generator.clone().unwrap_or_default();
  table.set("generator", typed_values_to_lua(lua, &generator.r#type, &generator.values)?)?;
  table.set("path", request.path.clone())?;
  table.set("example_value", field_value_to_lua(lua, &request.example_value)?)?;
  table.set(
    "plugin_configuration",
    plugin_configuration_to_lua(lua, &v2_plugin_configuration_to_v1(&request.plugin_configuration))?,
  )?;
  table.set("test_context", struct_to_lua(lua, &request.test_context)?)?;
  table.set("test_mode", test_mode_to_str(request.test_mode))?;
  Ok(table)
}

/// Reverse of [`generate_field_request_to_lua`] - the table a script passes to
/// `host_generate_field(entry_key, request)`.
fn lua_to_generate_field_request(lua: &Lua, table: Table) -> anyhow::Result<proto_v2::GenerateFieldRequest> {
  let (r#type, values) = lua_to_typed_values(lua, table.get("generator")?, "generator")?;
  let plugin_configuration: Option<Value> = table.get("plugin_configuration")?;
  let test_context: Option<Value> = table.get("test_context")?;
  let test_mode: Option<String> = table.get("test_mode")?;
  Ok(proto_v2::GenerateFieldRequest {
    key: table.get::<Option<String>>("key")?.unwrap_or_default(),
    generator: Some(proto_v2::Generator { r#type, values }),
    path: table.get::<Option<String>>("path")?.unwrap_or_default(),
    example_value: Some(lua_to_field_value(lua, table.get("example_value")?)?),
    plugin_configuration: v1_plugin_configuration_to_v2(
      lua_to_plugin_configuration(lua, plugin_configuration)?
    ),
    test_context: lua_to_struct(lua, test_context)?,
    test_mode: str_to_test_mode(test_mode.as_deref()),
  })
}

/// Parses the table a plugin's `generate_field` function returns: `{ value = ... }` or
/// `{ error = "..." }`.
fn lua_to_generate_field_response(lua: &Lua, table: Table) -> anyhow::Result<proto_v2::GenerateFieldResponse> {
  let error: Option<String> = table.get("error")?;
  if let Some(error) = error {
    return Ok(proto_v2::GenerateFieldResponse { error, value: None });
  }
  Ok(proto_v2::GenerateFieldResponse {
    error: String::new(),
    value: Some(lua_to_field_value(lua, table.get("value")?)?),
  })
}

/// Reverse of [`lua_to_generate_field_response`], so a script can return the result of a
/// `host_generate_field` call straight through as its own response.
fn generate_field_response_to_lua(lua: &Lua, response: &proto_v2::GenerateFieldResponse) -> mlua::Result<Table> {
  let table = lua.create_table()?;
  if !response.error.is_empty() {
    table.set("error", response.error.clone())?;
  } else {
    table.set("value", field_value_to_lua(lua, &response.value)?)?;
  }
  Ok(table)
}

/// Converts a plain Lua value back into a `google.protobuf.Struct`, the reverse of
/// [`struct_to_lua`]. A `nil` (or a non-map value, which a `Struct` cannot represent) becomes no
/// struct at all rather than an error.
fn lua_to_struct(lua: &Lua, value: Option<Value>) -> anyhow::Result<Option<prost_types::Struct>> {
  match value {
    None | Some(Value::Nil) => Ok(None),
    Some(value) => Ok(Some(to_proto_struct(&as_json_map(lua.from_value(value)?)))),
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
      let binary: Option<mlua::LuaString> = match &value {
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
    let match_fn: Function = lua
      .globals()
      .get("match_contents")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'match_contents' function"))?;
    let request_table = compare_request_to_lua(&lua, &request)?;
    let result: Table = match_fn
      .call_async(request_table)
      .await
      .map_err(|err| anyhow!("Lua match_contents() function failed - {}", err))?;
    lua_to_compare_response(result)
  }

  async fn configure_interaction(
    &self,
    request: ConfigureInteractionRequest,
  ) -> anyhow::Result<ConfigureInteractionResponse> {
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
        let test_mode = test_mode_to_str(request.test_mode);
        let result: Value = generate_fn
          .call_async((contents, generators, test_mode))
          .await
          .map_err(|err| anyhow!("Lua generate_content() function failed - {}", err))?;
        Ok(GenerateContentResponse {
          contents: lua_to_body(result)?,
        })
      }
    }
  }

  /// `match_field` and `generate_field` are optional globals - a plugin only defines them if it
  /// registered a `MATCHER` or `GENERATOR` catalogue entry. Reaching here without one is a real
  /// error (the driver resolved an entry the plugin registered), so it is reported rather than
  /// silently treated as a match.
  ///
  /// Called via `call_async` for the same reason as `match_contents`: the script may reach a host
  /// function, and mlua only allows an async host function to be called from a chain started with
  /// `call_async`.
  async fn match_field(
    &self,
    request: proto_v2::MatchFieldRequest,
  ) -> anyhow::Result<proto_v2::MatchFieldResponse> {
    let lua = self.runtime.lock().await;
    let match_fn: Function = lua
      .globals()
      .get("match_field")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'match_field' function"))?;
    let path = request.path.clone();
    let request_table = match_field_request_to_lua(&lua, &request)?;
    let result: Table = match_fn
      .call_async(request_table)
      .await
      .map_err(|err| anyhow!("Lua match_field() function failed - {}", err))?;
    lua_to_match_field_response(result, &path)
  }

  /// See [`LuaPactPlugin::match_field`].
  async fn generate_field(
    &self,
    request: proto_v2::GenerateFieldRequest,
  ) -> anyhow::Result<proto_v2::GenerateFieldResponse> {
    let lua = self.runtime.lock().await;
    let generate_fn: Function = lua
      .globals()
      .get("generate_field")
      .map_err(|_| anyhow!("Lua plugin does not define a global 'generate_field' function"))?;
    let request_table = generate_field_request_to_lua(&lua, &request)?;
    let result: Table = generate_fn
      .call_async(request_table)
      .await
      .map_err(|err| anyhow!("Lua generate_field() function failed - {}", err))?;
    lua_to_generate_field_response(&lua, result)
  }

  async fn start_mock_server(
    &self,
    request: StartMockServerRequest,
  ) -> anyhow::Result<StartMockServerResponse> {
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
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
    let lua = self.runtime.lock().await;
    let update_fn: Option<Function> = lua.globals().get("update_catalogue")?;
    if let Some(update_fn) = update_fn {
      let table = lua.create_table()?;
      for entry in &request.catalogue {
        let entry_table = lua.create_table()?;
        // An entry type this driver doesn't understand is skipped rather than passed to the
        // script as some other type it isn't - see `register_plugin_entries`.
        let Some(entry_type) = CatalogueEntryType::from_proto_value(entry.r#type) else {
          warn!("Not passing catalogue entry '{}' to the plugin: {} is not a catalogue entry type this driver understands",
            entry.key, entry.r#type);
          continue;
        };
        entry_table.set("entryType", entry_type.as_proto_name())?;
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
    let lua = plugin.runtime.blocking_lock();
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
    let lua = plugin.runtime.blocking_lock();
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
    let lua = plugin.runtime.blocking_lock();
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
    let lua = plugin.runtime.lock().await;
    let entries = call_init(&lua, "test", "0.0.0").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "jwt");
    assert_eq!(entries[0].r#type, catalogue_entry::EntryType::ContentMatcher as i32);
    assert_eq!(entries[1].r#type, catalogue_entry::EntryType::ContentGenerator as i32);
  }

  #[tokio::test]
  async fn init_accepts_matcher_and_generator_catalogue_entries() {
    // A field-level plugin registers MATCHER/GENERATOR entries rather than content ones. GENERATOR
    // only exists on the V2 enum, so this also covers the entry type surviving the trip through
    // the V1-shaped CatalogueEntry message the driver uses internally.
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        function init(implementation, version)
          return {
            { entryType = "MATCHER", key = "creditcard", values = { ["config-key"] = "brand" } },
            { entryType = "GENERATOR", key = "creditcard" }
          }
        end
      "#,
    ).unwrap();

    let manifest = lua_manifest(plugin_dir.path(), "field-entries-test");
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().await;
    let entries = call_init(&lua, "test", "0.0.0").unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "creditcard");
    assert_eq!(entries[0].r#type, CatalogueEntryType::MATCHER.to_proto_value());
    assert_eq!(entries[0].values.get("config-key"), Some(&"brand".to_string()));
    assert_eq!(entries[1].r#type, CatalogueEntryType::GENERATOR.to_proto_value());
  }

  #[tokio::test]
  async fn init_rejects_an_unknown_catalogue_entry_type() {
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      r#"
        function init(implementation, version)
          return { { entryType = "NOT_AN_ENTRY_TYPE", key = "nope" } }
        end
      "#,
    ).unwrap();

    let manifest = lua_manifest(plugin_dir.path(), "bad-entry-type-test");
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().await;

    let error = call_init(&lua, "test", "0.0.0").unwrap_err().to_string();
    assert!(error.contains("NOT_AN_ENTRY_TYPE"), "unexpected error: {}", error);
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

  fn lua_manifest(plugin_dir: &std::path::Path, name: &str) -> PactPluginManifest {
    PactPluginManifest {
      plugin_dir: plugin_dir.to_string_lossy().to_string(),
      plugin_interface_version: 1,
      name: name.to_string(),
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

  fn core_matcher_entry(key: &str) -> crate::catalogue_manager::CatalogueEntry {
    crate::catalogue_manager::CatalogueEntry {
      entry_type: crate::catalogue_manager::CatalogueEntryType::CONTENT_MATCHER,
      provider_type: crate::catalogue_manager::CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: HashMap::new()
    }
  }

  fn core_generator_entry(key: &str) -> crate::catalogue_manager::CatalogueEntry {
    crate::catalogue_manager::CatalogueEntry {
      entry_type: crate::catalogue_manager::CatalogueEntryType::CONTENT_GENERATOR,
      provider_type: crate::catalogue_manager::CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: HashMap::new()
    }
  }

  struct FixedErrorCoreMatcher;

  #[async_trait]
  impl crate::core_capabilities::CoreContentMatcher for FixedErrorCoreMatcher {
    async fn compare_contents(&self, _request: CompareContentsRequest) -> anyhow::Result<CompareContentsResponse> {
      Ok(CompareContentsResponse {
        error: "core matcher says no".to_string(),
        type_mismatch: None,
        results: HashMap::new(),
      })
    }
  }

  #[tokio::test]
  async fn match_contents_calls_host_compare_contents_for_a_registered_core_capability() {
    let key = "match_contents_calls_host_compare_contents_for_a_registered_core_capability";
    crate::catalogue_manager::register_core_entries(&vec![core_matcher_entry(key)]);
    crate::core_capabilities::register_core_content_matcher(key, Arc::new(FixedErrorCoreMatcher));

    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      format!(r#"
        function match_contents(request)
          return host_compare_contents("{key}", request)
        end
      "#, key = key),
    ).unwrap();
    let manifest = lua_manifest(plugin_dir.path(), "host-compare-contents-test");
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let compare_request = CompareContentsRequest {
      expected: None,
      actual: None,
      allow_unexpected_keys: false,
      rules: HashMap::new(),
      plugin_configuration: None,
    };
    let response = plugin.compare_contents(compare_request).await.unwrap();

    crate::core_capabilities::deregister_core_content_matcher(key);

    assert_eq!(response.error, "core matcher says no");
  }

  #[tokio::test]
  async fn match_contents_surfaces_a_clear_error_when_host_compare_contents_targets_an_unregistered_entry() {
    let key = "match_contents_surfaces_a_clear_error_when_host_compare_contents_targets_an_unregistered_entry";
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      format!(r#"
        function match_contents(request)
          return host_compare_contents("{key}", request)
        end
      "#, key = key),
    ).unwrap();
    let manifest = lua_manifest(plugin_dir.path(), "host-compare-contents-missing-test");
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let compare_request = CompareContentsRequest {
      expected: None,
      actual: None,
      allow_unexpected_keys: false,
      rules: HashMap::new(),
      plugin_configuration: None,
    };
    let err = plugin.compare_contents(compare_request).await
      .expect_err("expected an error when the target entry is not registered");
    assert!(
      err.to_string().contains("No catalogue entry found"),
      "unexpected error message: {}", err
    );
  }

  struct FixedCoreGenerator;

  #[async_trait]
  impl crate::core_capabilities::CoreContentGenerator for FixedCoreGenerator {
    async fn generate_content(&self, _request: GenerateContentRequest) -> anyhow::Result<GenerateContentResponse> {
      Ok(GenerateContentResponse {
        contents: Some(Body {
          content_type: "text/plain".to_string(),
          content: Some(b"generated by the host".to_vec()),
          content_type_hint: body::ContentTypeHint::Default as i32,
        }),
      })
    }
  }

  #[tokio::test]
  async fn generate_content_calls_host_generate_content_for_a_registered_core_capability() {
    let key = "generate_content_calls_host_generate_content_for_a_registered_core_capability";
    crate::catalogue_manager::register_core_entries(&vec![core_generator_entry(key)]);
    crate::core_capabilities::register_core_content_generator(key, Arc::new(FixedCoreGenerator));

    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(
      plugin_dir.path().join("entry.lua"),
      format!(r#"
        function generate_content(contents, generators, test_mode)
          return host_generate_content("{key}", contents, generators, test_mode)
        end
      "#, key = key),
    ).unwrap();
    let manifest = lua_manifest(plugin_dir.path(), "host-generate-content-test");
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();

    let request = GenerateContentRequest {
      contents: Some(Body {
        content_type: "text/plain".to_string(),
        content: Some(b"original".to_vec()),
        content_type_hint: body::ContentTypeHint::Default as i32,
      }),
      .. GenerateContentRequest::default()
    };
    let response = plugin.generate_content(request).await.unwrap();

    crate::core_capabilities::deregister_core_content_generator(key);

    assert_eq!(response.contents.unwrap().content, Some(b"generated by the host".to_vec()));
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

    let lua = plugin.runtime.lock().await;
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

    let lua = plugin.runtime.lock().await;
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

    let lua = plugin.runtime.lock().await;
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

    let lua = plugin.runtime.lock().await;
    let captured: Table = lua.globals().get("VERIFY_REQUEST").unwrap();
    let interaction_data: Table = captured.get("interaction_data").unwrap();
    let body: Table = interaction_data.get("body").unwrap();
    assert_eq!(body.get::<mlua::LuaString>("contents").unwrap().to_str().unwrap(), "request-body");
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

  // ---- Field-level matchers and generators (proposal 006) ----

  fn creditcard_manifest() -> PactPluginManifest {
    // See jwt_manifest() for why this path is deliberately not canonicalized.
    let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/creditcard");
    assert!(plugin_dir.exists(), "plugins/creditcard directory should exist at {:?}", plugin_dir);
    PactPluginManifest {
      plugin_dir: plugin_dir.to_string_lossy().to_string(),
      plugin_interface_version: 2,
      name: "creditcard".to_string(),
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

  fn text_field(value: &str) -> proto_v2::FieldValue {
    proto_v2::FieldValue {
      value: Some(proto_v2::field_value::Value::StringValue(value.to_string()))
    }
  }

  /// A `MatchFieldRequest` for the `creditcard` rule, optionally configured with a brand.
  fn creditcard_match_request(brand: Option<&str>, expected: &str, actual: &str) -> proto_v2::MatchFieldRequest {
    let values = brand.map(|brand| {
      let mut fields = HashMap::new();
      fields.insert("brand".to_string(), serde_json::Value::String(brand.to_string()));
      to_proto_struct(&fields)
    });
    proto_v2::MatchFieldRequest {
      key: "creditcard".to_string(),
      rule: Some(proto_v2::MatchingRule { r#type: "creditcard".to_string(), values }),
      path: "$.card.number".to_string(),
      mismatch_type: "body".to_string(),
      expected: Some(text_field(expected)),
      actual: Some(text_field(actual)),
      plugin_configuration: None,
      test_context: None,
    }
  }

  fn creditcard_generate_request(brand: Option<&str>, example: &str) -> proto_v2::GenerateFieldRequest {
    let values = brand.map(|brand| {
      let mut fields = HashMap::new();
      fields.insert("brand".to_string(), serde_json::Value::String(brand.to_string()));
      to_proto_struct(&fields)
    });
    proto_v2::GenerateFieldRequest {
      key: "creditcard".to_string(),
      generator: Some(proto_v2::Generator { r#type: "creditcard".to_string(), values }),
      path: "$.card.number".to_string(),
      example_value: Some(text_field(example)),
      plugin_configuration: None,
      test_context: None,
      test_mode: proto_v2::generate_content_request::TestMode::Consumer as i32,
    }
  }

  #[tokio::test]
  async fn creditcard_plugin_registers_a_matcher_and_a_generator_under_the_same_key() {
    let manifest = creditcard_manifest();
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    let lua = plugin.runtime.lock().await;
    let entries = call_init(&lua, "test", "0.0.0").unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "creditcard");
    assert_eq!(entries[0].r#type, proto_v2::catalogue_entry::EntryType::Matcher as i32);
    assert_eq!(entries[1].key, "creditcard");
    assert_eq!(entries[1].r#type, proto_v2::catalogue_entry::EntryType::Generator as i32);
    // The values key that maps a single positional config argument in a rule definition
    assert_eq!(entries[0].values.get("config-key"), Some(&"brand".to_string()));
  }

  #[tokio::test]
  async fn creditcard_plugin_accepts_a_valid_card_number() {
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    let response = plugin
      .match_field(creditcard_match_request(Some("visa"), "4111111111111111", "4012888888881881"))
      .await
      .unwrap();

    assert_eq!(response.error, "");
    assert!(response.mismatches.is_empty(), "expected no mismatches, got {:?}", response.mismatches);
  }

  #[tokio::test]
  async fn creditcard_plugin_reports_a_number_that_fails_the_luhn_check() {
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    let response = plugin
      .match_field(creditcard_match_request(None, "4111111111111111", "4111111111111112"))
      .await
      .unwrap();

    assert_eq!(response.error, "");
    assert_eq!(response.mismatches.len(), 1);
    let mismatch = &response.mismatches[0];
    assert!(
      mismatch.mismatch.contains("Luhn check"),
      "unexpected mismatch description: {}", mismatch.mismatch
    );
    // The plugin places its own mismatches, echoing back the path and part it was given
    assert_eq!(mismatch.path, "$.card.number");
    assert_eq!(mismatch.mismatch_type, "body");
    assert_eq!(mismatch.expected.as_deref(), Some("4111111111111111".as_bytes()));
    assert_eq!(mismatch.actual.as_deref(), Some("4111111111111112".as_bytes()));
  }

  #[tokio::test]
  async fn creditcard_plugin_reports_a_number_from_the_wrong_brand() {
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    // A valid Visa number, but the rule asks for a Mastercard
    let response = plugin
      .match_field(creditcard_match_request(Some("mastercard"), "5555555555554444", "4012888888881881"))
      .await
      .unwrap();

    assert_eq!(response.mismatches.len(), 1);
    assert!(
      response.mismatches[0].mismatch.contains("Mastercard"),
      "unexpected mismatch description: {}", response.mismatches[0].mismatch
    );
  }

  #[tokio::test]
  async fn creditcard_plugin_reports_a_misconfigured_brand_as_an_error_not_a_mismatch() {
    // The test author's mistake, not the provider's - so it fails the test outright rather than
    // being reported as the provider sending the wrong value.
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    let response = plugin
      .match_field(creditcard_match_request(Some("amx"), "4111111111111111", "4111111111111111"))
      .await
      .unwrap();

    assert!(
      response.error.contains("'amx' is not a credit card brand"),
      "unexpected error: {}", response.error
    );
    assert!(response.mismatches.is_empty());
  }

  #[tokio::test]
  async fn creditcard_plugin_generates_a_number_for_the_configured_brand() {
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    let response = plugin
      .generate_field(creditcard_generate_request(Some("amex"), "4111111111111111"))
      .await
      .unwrap();

    assert_eq!(response.error, "");
    let generated = match response.value.and_then(|value| value.value) {
      Some(proto_v2::field_value::Value::StringValue(value)) => value,
      other => panic!("expected a generated string value, got {:?}", other)
    };
    assert_eq!(generated.len(), 15, "an Amex number has 15 digits, got '{}'", generated);
    assert!(generated.starts_with("34") || generated.starts_with("37"), "got '{}'", generated);

    // And the number it generated is one it will accept back
    let match_response = plugin
      .match_field(creditcard_match_request(Some("amex"), "371449635398431", &generated))
      .await
      .unwrap();
    assert!(
      match_response.mismatches.is_empty(),
      "the plugin should accept its own generated number, got {:?}", match_response.mismatches
    );
  }

  #[tokio::test]
  async fn creditcard_plugin_reports_a_generator_error() {
    let plugin = start_lua_plugin(&creditcard_manifest(), "test-instance".to_string()).unwrap();

    let response = plugin
      .generate_field(creditcard_generate_request(Some("amx"), "4111111111111111"))
      .await
      .unwrap();

    assert!(response.error.contains("amx"), "unexpected error: {}", response.error);
    assert!(response.value.is_none());
  }

  /// Starts a Lua plugin whose entry point is the given script source.
  fn start_field_plugin(name: &str, script: &str) -> (tempdir::TempDir, LuaPactPlugin) {
    let plugin_dir = tempdir::TempDir::new("lua-plugin-test").unwrap();
    std::fs::write(plugin_dir.path().join("entry.lua"), script).unwrap();
    let manifest = lua_manifest(plugin_dir.path(), name);
    let plugin = start_lua_plugin(&manifest, "test-instance".to_string()).unwrap();
    // The temp dir is returned so it outlives the plugin - dropping it deletes the script
    (plugin_dir, plugin)
  }

  #[tokio::test]
  async fn each_field_value_type_survives_the_round_trip_through_lua() {
    use proto_v2::field_value::Value as FieldValue;

    let (_dir, plugin) = start_field_plugin(
      "field-value-round-trip-test",
      r#"
        function generate_field(request)
          return { value = request.example_value }
        end
      "#,
    );

    let values = vec![
      FieldValue::NullValue(0),
      FieldValue::BooleanValue(true),
      FieldValue::StringValue("4111111111111111".to_string()),
      FieldValue::IntegerValue(100),
      FieldValue::DecimalValue(100.5),
      FieldValue::BinaryValue(vec![0, 159, 146, 150]),
    ];

    for value in values {
      let request = proto_v2::GenerateFieldRequest {
        generator: Some(proto_v2::Generator::default()),
        example_value: Some(proto_v2::FieldValue { value: Some(value.clone()) }),
        .. proto_v2::GenerateFieldRequest::default()
      };
      let response = plugin.generate_field(request).await.unwrap();
      assert_eq!(
        response.value.and_then(|response| response.value), Some(value.clone()),
        "{:?} did not survive the round trip through Lua", value
      );
    }
  }

  #[tokio::test]
  async fn a_whole_number_reaches_lua_as_an_integer_and_a_decimal_as_a_float() {
    // The distinction the integer, decimal and type rules are built on. Lua 5.4 has separate
    // integer and float subtypes, so it can be checked from inside the script itself.
    let (_dir, plugin) = start_field_plugin(
      "field-value-lua-type-test",
      r#"
        function generate_field(request)
          return { value = math.type(request.example_value) }
        end
      "#,
    );

    let lua_type_of = async |value: proto_v2::field_value::Value| {
      let request = proto_v2::GenerateFieldRequest {
        example_value: Some(proto_v2::FieldValue { value: Some(value) }),
        .. proto_v2::GenerateFieldRequest::default()
      };
      match plugin.generate_field(request).await.unwrap().value.and_then(|value| value.value) {
        Some(proto_v2::field_value::Value::StringValue(value)) => value,
        other => panic!("expected math.type() to return a string, got {:?}", other)
      }
    };

    assert_eq!(lua_type_of(proto_v2::field_value::Value::IntegerValue(100)).await, "integer");
    assert_eq!(lua_type_of(proto_v2::field_value::Value::DecimalValue(100.0)).await, "float");
  }

  #[tokio::test]
  async fn a_mismatch_that_does_not_place_itself_is_reported_against_the_request_path() {
    let (_dir, plugin) = start_field_plugin(
      "field-mismatch-path-test",
      r#"
        function match_field(request)
          return { mismatches = { "not a card number" } }
        end
      "#,
    );

    let response = plugin
      .match_field(creditcard_match_request(None, "4111111111111111", "nope"))
      .await
      .unwrap();

    assert_eq!(response.mismatches.len(), 1);
    assert_eq!(response.mismatches[0].mismatch, "not a card number");
    assert_eq!(response.mismatches[0].path, "$.card.number");
  }

  #[tokio::test]
  async fn a_plugin_that_does_not_define_the_field_functions_says_so() {
    let (_dir, plugin) = start_field_plugin("field-functions-missing-test", "-- nothing here");

    let match_error = plugin.match_field(proto_v2::MatchFieldRequest::default()).await
      .expect_err("expected an error when the plugin does not define match_field");
    assert!(
      match_error.to_string().contains("does not define a global 'match_field' function"),
      "unexpected error: {}", match_error
    );

    let generate_error = plugin.generate_field(proto_v2::GenerateFieldRequest::default()).await
      .expect_err("expected an error when the plugin does not define generate_field");
    assert!(
      generate_error.to_string().contains("does not define a global 'generate_field' function"),
      "unexpected error: {}", generate_error
    );
  }

  fn core_field_matcher_entry(key: &str) -> crate::catalogue_manager::CatalogueEntry {
    crate::catalogue_manager::CatalogueEntry {
      entry_type: crate::catalogue_manager::CatalogueEntryType::MATCHER,
      provider_type: crate::catalogue_manager::CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: HashMap::new()
    }
  }

  fn core_field_generator_entry(key: &str) -> crate::catalogue_manager::CatalogueEntry {
    crate::catalogue_manager::CatalogueEntry {
      entry_type: crate::catalogue_manager::CatalogueEntryType::GENERATOR,
      provider_type: crate::catalogue_manager::CatalogueEntryProviderType::CORE,
      plugin: None,
      key: key.to_string(),
      values: HashMap::new()
    }
  }

  /// A core field matcher that reports what it was handed, so the test can check the request the
  /// script built actually arrived intact.
  struct EchoCoreFieldMatcher;

  #[async_trait]
  impl crate::core_capabilities::CoreFieldMatcher for EchoCoreFieldMatcher {
    async fn match_field(&self, request: proto_v2::MatchFieldRequest) -> anyhow::Result<proto_v2::MatchFieldResponse> {
      let rule = request.rule.unwrap_or_default();
      Ok(proto_v2::MatchFieldResponse {
        error: String::new(),
        mismatches: vec![proto_v2::ContentMismatch {
          expected: None,
          actual: None,
          mismatch: format!("core matcher saw rule '{}' at {}", rule.r#type, request.path),
          path: request.path,
          diff: String::new(),
          mismatch_type: request.mismatch_type,
        }]
      })
    }
  }

  #[tokio::test]
  async fn match_field_calls_host_match_field_for_a_registered_core_capability() {
    // A plugin that owns a content type delegating one value inside it to a standard Pact rule,
    // rather than reimplementing it - the point of the host callbacks (proposal 006 section 7).
    let key = "match_field_calls_host_match_field_for_a_registered_core_capability";
    crate::catalogue_manager::register_core_entries(&vec![core_field_matcher_entry(key)]);
    crate::core_capabilities::register_core_field_matcher(key, Arc::new(EchoCoreFieldMatcher));

    let (_dir, plugin) = start_field_plugin(
      "host-match-field-test",
      &format!(r#"
        function match_field(request)
          return host_match_field("{key}", request)
        end
      "#, key = key),
    );

    let response = plugin
      .match_field(creditcard_match_request(Some("visa"), "4111111111111111", "4012888888881881"))
      .await
      .unwrap();

    crate::core_capabilities::deregister_core_field_matcher(key);

    assert_eq!(response.mismatches.len(), 1);
    assert_eq!(
      response.mismatches[0].mismatch,
      "core matcher saw rule 'creditcard' at $.card.number"
    );
    assert_eq!(response.mismatches[0].mismatch_type, "body");
  }

  #[tokio::test]
  async fn host_match_field_surfaces_a_clear_error_when_the_entry_is_not_registered() {
    let key = "host_match_field_surfaces_a_clear_error_when_the_entry_is_not_registered";
    let (_dir, plugin) = start_field_plugin(
      "host-match-field-missing-test",
      &format!(r#"
        function match_field(request)
          return host_match_field("{key}", request)
        end
      "#, key = key),
    );

    let err = plugin.match_field(creditcard_match_request(None, "4111111111111111", "4111111111111111"))
      .await
      .expect_err("expected an error when the target entry is not registered");
    assert!(
      err.to_string().contains("No catalogue entry found"),
      "unexpected error message: {}", err
    );
  }

  struct FixedCoreFieldGenerator;

  #[async_trait]
  impl crate::core_capabilities::CoreFieldGenerator for FixedCoreFieldGenerator {
    async fn generate_field(&self, _request: proto_v2::GenerateFieldRequest) -> anyhow::Result<proto_v2::GenerateFieldResponse> {
      Ok(proto_v2::GenerateFieldResponse {
        error: String::new(),
        value: Some(text_field("generated by the host"))
      })
    }
  }

  #[tokio::test]
  async fn generate_field_calls_host_generate_field_for_a_registered_core_capability() {
    let key = "generate_field_calls_host_generate_field_for_a_registered_core_capability";
    crate::catalogue_manager::register_core_entries(&vec![core_field_generator_entry(key)]);
    crate::core_capabilities::register_core_field_generator(key, Arc::new(FixedCoreFieldGenerator));

    let (_dir, plugin) = start_field_plugin(
      "host-generate-field-test",
      &format!(r#"
        function generate_field(request)
          return host_generate_field("{key}", request)
        end
      "#, key = key),
    );

    let response = plugin
      .generate_field(creditcard_generate_request(Some("visa"), "4111111111111111"))
      .await
      .unwrap();

    crate::core_capabilities::deregister_core_field_generator(key);

    assert_eq!(response.error, "");
    assert_eq!(
      response.value.and_then(|value| value.value),
      Some(proto_v2::field_value::Value::StringValue("generated by the host".to_string()))
    );
  }
}
