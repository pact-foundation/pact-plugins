use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use itertools::Itertools;
use mlua::{FromLua, Function, IntoLua, Lua, LuaSerdeExt, Table, UserData, Value};
use mlua::prelude::LuaValue;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::pact::Pact;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use rsa::pkcs1v15::SigningKey;
use rsa::RsaPrivateKey;
use rsa::pkcs1::{DecodeRsaPrivateKey, LineEnding};
use rsa::pkcs8::EncodePublicKey;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use tracing::debug;

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryType, register_plugin_entries};
use crate::content::{InteractionContents, PluginConfiguration};
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::plugin_models::{CompareContentRequest, CompareContentResult, GenerateContentRequest, PactPlugin, PactPluginManifest};
use crate::verification::{InteractionVerificationData, InteractionVerificationResult};

impl<'lua> FromLua<'lua> for CatalogueEntry  {
  fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
    match &value {
      Value::Table(_t) => lua.from_value(value),
      _ => Err(mlua::Error::external(anyhow!("Invalid type for a catalog entry, a table is required")))
    }
  }
}

#[derive(Debug, Clone, Default)]
struct ConfigureInteractionResult {
  pub contents: Vec<InteractionContents>,
  pub plugin_config: Option<PluginConfiguration>
}

impl<'lua> FromLua<'lua> for ConfigureInteractionResult  {
  fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
    match &value {
      Value::Table(t) => {
        let plugin_config = if let Ok(plugin_config) = t.get::<&str, Value>("plugin_config") {
          Some(lua.from_value(plugin_config)?)
        } else {
          None
        };

        let contents = if let Ok(contents) = t.get("contents") {
          match contents {
            Value::Table(t) => {
              let mut contents = vec![];
              for pair in t.pairs() {
                let (_, v): (Value<'_>, InteractionContents) = pair?;
                contents.push(v);
              }
              contents
            }
            _ => return Err(mlua::Error::external(anyhow!("Invalid type for Contents, a table is required")))
          }
        } else {
          vec![]
        };

        Ok(ConfigureInteractionResult {
          contents,
          plugin_config,
          .. ConfigureInteractionResult::default()
        })
      },
      _ => Err(mlua::Error::external(anyhow!("Invalid type for ConfigureInteractionResult, a table is required")))
    }
  }
}

impl<'lua> FromLua<'lua> for InteractionContents {
  fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
    if let Some(table) = value.as_table() {
      let part_name = table.get("part_name").unwrap_or_default();

      let body = table.get("body").ok().map(|value: Value<'_>| {
        match &value {
          Value::String(s) => s.to_string_lossy().to_string().into(),
          Value::Table(t) => lua.from_value::<OptionalBody>(value).ok().unwrap_or_default(),
          _ => OptionalBody::Empty
        }
      }).unwrap_or_default();

      Ok(InteractionContents {
        part_name,
        body,
        ..InteractionContents::default()
      })
    } else {
      Err(mlua::Error::external(anyhow!("Invalid type for InteractionContents, a table is required")))
    }
  }
}

/// Plugin that supports Lua scripts
#[derive(Debug, Clone)]
pub struct LuaPlugin {
  runtime: Arc<Mutex<Lua>>,
  manifest: PactPluginManifest
}

impl LuaPlugin {
  /// Loads and executes the init script for the plugin
  pub fn load_init_script(&self, init_script: &PathBuf) -> anyhow::Result<()> {
    let runtime = self.runtime.lock().unwrap();
    let globals = runtime.globals();

    if let Some(script_dir) = init_script.parent() {
      let package: Table = globals.get("package")?;
      let path: String = package.get("path")?;
      package.set("path", format!("{}/?.lua;{}", script_dir.to_string_lossy(), path))?;
    }

    let name = self.manifest.name.clone();
    let logger_fn = runtime.create_function(move |_, message: String| {
      debug!("Plugin({}) || {}", name, message);
      Ok(())
    })?;
    globals.set("logger", logger_fn)?;

    let rsa_sig_fn = runtime.create_function(move |_, (data, key): (String, String)| {
      debug!("Decoding private key as PKCS8 format");
      let private_key = RsaPrivateKey::from_pkcs1_pem(key.as_str())
        .map_err(|err| mlua::Error::external(anyhow!("Failed to decode the private RSA key: {}", err)))?;
      debug!("Creating signing key for RSASSA_PKCS1_V1_5_SHA_512");
      let signing_key = SigningKey::<Sha512>::new(private_key);
      let mut rng = rand::thread_rng();
      let signature = signing_key.try_sign_with_rng(&mut rng, data.as_bytes())
        .map_err(|err| mlua::Error::external(anyhow!("Failed to sign the data with the signing key: {}", err)))?;
      let sig_bytes = signature.to_bytes();
      Ok(BASE64.encode(sig_bytes))
    })?;
    globals.set("rsa_sign", rsa_sig_fn)?;

    let rsa_public_key_fn = runtime.create_function(move |_, key: String| {
      debug!("Decoding private key as PKCS8 format");
      let private_key = RsaPrivateKey::from_pkcs1_pem(key.as_str())
        .map_err(|err| mlua::Error::external(anyhow!("Failed to decode the private RSA key: {}", err)))?;
      let public_key = private_key.to_public_key();
      public_key.to_public_key_pem(LineEnding::LF)
        .map_err(|err| mlua::Error::external(anyhow!("Failed to encode the public RSA key: {}", err)))
    })?;
    globals.set("rsa_public_key", rsa_public_key_fn)?;

    let script = read_to_string(init_script)?;
    runtime.load(script).exec()
      .map_err(|err| anyhow!("Failed to execute Lua init script: {}", err))
  }

  /// Calls the plugins init function
  pub fn init(&self) -> anyhow::Result<()> {
    let result = {
      let runtime = self.runtime.lock().unwrap();
      let globals = runtime.globals();
      let init_fn: Function = globals.get("init")?;
      init_fn.call::<_, HashMap<u16, CatalogueEntry>>(("plugin-driver-rust", option_env!("CARGO_PKG_VERSION").unwrap_or("0")))
        .map_err(|err| anyhow!("Failed to execute Lua init function: {}", err))?
    };


    register_plugin_entries(&self.manifest, result.values()
      .map(|v| CatalogueEntry { plugin: Some(self.manifest.clone()), .. v.clone() })
      .collect_vec()
    );

    Ok(())
  }
}

#[async_trait]
impl PactPlugin for LuaPlugin {
  fn manifest(&self) -> PactPluginManifest {
    self.manifest.clone()
  }

  fn kill(&self) {
    // no op
  }

  fn update_access(&mut self) {
    // no op
  }

  fn drop_access(&mut self) -> usize {
    // no op
    1
  }

  fn boxed(&self) -> Box<dyn PactPlugin + Send + Sync> {
    Box::new(self.clone())
  }

  fn arced(&self) -> Arc<dyn PactPlugin + Send + Sync> {
    Arc::new(self.clone())
  }

  async fn publish_updated_catalogue(&self, _catalogue: &[CatalogueEntry]) -> anyhow::Result<()> {
    // no op
    Ok(())
  }

  async fn generate_contents(&self, request: GenerateContentRequest) -> anyhow::Result<OptionalBody> {
    todo!()
  }

  async fn match_contents(&self, request: CompareContentRequest) -> anyhow::Result<CompareContentResult> {
    todo!()
  }

  async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: &HashMap<String, serde_json::Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)> {
    {
      let runtime = self.runtime.lock().unwrap();
      let globals = runtime.globals();
      let table = runtime.create_table()?;
      for (key, value) in definition {
        table.set(key.as_str(), to_lua_value(value, &runtime)?)?;
      }
      let configure_interaction_fn: Function = globals.get("configure_interaction")?;
      let result = configure_interaction_fn.call::<_, ConfigureInteractionResult>((content_type.to_string().as_str(), table))
        .map_err(|err| anyhow!("Failed to execute Lua configure_interaction function: {}", err))?;
      dbg!(result);
    }
    todo!()
  }

  async fn verify_interaction(&self, _pact: &V4Pact, _interaction: &(dyn V4Interaction + Send + Sync), _verification_data: &InteractionVerificationData, config: &HashMap<String, serde_json::Value>) -> anyhow::Result<InteractionVerificationResult> {
    unimplemented!()
  }

  async fn prepare_interaction_for_verification(&self, _pact: &V4Pact, _interaction: &(dyn V4Interaction + Send + Sync), _context: &HashMap<String, serde_json::Value>) -> anyhow::Result<InteractionVerificationData> {
    unimplemented!()
  }

  async fn start_mock_server(&self, _config: &MockServerConfig, _pact: Box<dyn Pact + Send + Sync>, _test_context: HashMap<String, serde_json::Value>) -> anyhow::Result<MockServerDetails> {
    unimplemented!()
  }

  async fn get_mock_server_results(&self, _mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>> {
    unimplemented!()
  }

  async fn shutdown_mock_server(&self, _mock_server_key: &str) -> anyhow::Result<Vec<MockServerResults>> {
    unimplemented!()
  }
}

fn to_lua_value<'a>(value: &'a serde_json::Value, lua: &'a Lua) -> anyhow::Result<Value<'a>> {
  match value {
    serde_json::Value::Null => Err(anyhow!("Can not convert a NULL value to a Lua value")),
    serde_json::Value::Bool(b) => lua.to_value(b).map_err(|err| anyhow!("Failed to convert value: {}", err)),
    serde_json::Value::Number(n) => if n.is_f64() {
      lua.to_value(&n.as_f64().unwrap()).map_err(|err| anyhow!("Failed to convert value: {}", err))
    } else if n.is_i64() {
      lua.to_value(&n.as_i64().unwrap()).map_err(|err| anyhow!("Failed to convert value: {}", err))
    } else {
      lua.to_value(&n.as_u64().unwrap()).map_err(|err| anyhow!("Failed to convert value: {}", err))
    }
    serde_json::Value::String(s) => lua.to_value(s).map_err(|err| anyhow!("Failed to convert value: {}", err)),
    serde_json::Value::Array(a) => {
      lua.to_value(a).map_err(|err| anyhow!("Failed to convert value: {}", err))
    }
    serde_json::Value::Object(o) => {
      lua.to_value(o).map_err(|err| anyhow!("Failed to convert value: {}", err))
    }
  }
}

/// Starts a Lua plugin by wrapping a Lua runtime
pub(crate) fn start_lua_plugin(manifest: &PactPluginManifest) -> anyhow::Result<LuaPlugin> {
  debug!("Starting Lua plugin with manifest {:?}", manifest);

  let mut path = PathBuf::from(&manifest.entry_point);
  if !path.is_absolute() || !path.exists() {
    path = PathBuf::from(manifest.plugin_dir.clone()).join(path);
  }
  debug!("Starting plugin using {:?}", &path);

  let plugin = LuaPlugin {
    runtime: Arc::new(Mutex::new(Lua::new())),
    manifest: manifest.clone()
  };

  plugin.load_init_script(&path)?;

  Ok(plugin)
}
