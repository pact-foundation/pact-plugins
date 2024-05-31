use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE as BASE64;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64_NO_PAD;
use bytes::Bytes;
use itertools::Itertools;
use maplit::hashmap;
use mlua::{FromLua, Function, IntoLua, Lua, LuaSerdeExt, Table, UserData, Value};
use mlua::prelude::LuaValue;
use pact_models::bodies::OptionalBody;
use pact_models::content_types::ContentType;
use pact_models::matchingrules::RuleList;
use pact_models::pact::Pact;
use pact_models::path_exp::DocPath;
use pact_models::prelude::v4::V4Pact;
use pact_models::v4::interaction::V4Interaction;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use rsa::pkcs1::{DecodeRsaPrivateKey, LineEnding};
use rsa::pkcs8::{EncodePublicKey, DecodePublicKey};
use rsa::signature::{RandomizedSigner, SignatureEncoding, Verifier};
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use tracing::{debug, error, trace};

use crate::catalogue_manager::{CatalogueEntry, CatalogueEntryType, register_plugin_entries};
use crate::content::{ContentMismatch, InteractionContents, PluginConfiguration};
use crate::mock_server::{MockServerConfig, MockServerDetails, MockServerResults};
use crate::plugin_models::{CompareContentRequest, CompareContentResult, GenerateContentRequest, PactPlugin, PactPluginManifest, PluginInteractionConfig};
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
          Value::Table(t) => if let Ok(contents) = t.get::<_, Value>("contents") {
            let content_type = if let Ok(s) = t.get::<_, String>("content_type") {
              Some(ContentType::from(s))
            } else {
              None
            };
            let content_type_hint = if let Ok(s) = t.get::<_, Value>("content_type_hint") {
              lua.from_value(s).ok()
            } else {
              None
            };
            if let Ok(bytes) = lua.from_value::<Vec<u8>>(contents.clone()) {
              OptionalBody::Present(Bytes::from(bytes), content_type, content_type_hint)
            } else if let Ok(s) = lua.from_value::<String>(contents) {
              OptionalBody::Present(Bytes::copy_from_slice(s.as_bytes()), content_type, content_type_hint)
            } else {
              OptionalBody::Empty
            }
          } else {
            OptionalBody::Empty
          },
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

impl<'lua> IntoLua<'lua> for CompareContentRequest {
  fn into_lua(self, lua: &'lua Lua) -> mlua::Result<Value<'lua>> {
    let table = lua.create_table()?;
    table.set("expected_contents", body_to_lua(lua, &self.expected_contents)?)?;
    table.set("actual_contents", body_to_lua(lua, &self.actual_contents)?)?;
    table.set("allow_unexpected_keys", self.allow_unexpected_keys)?;
    // TODO: pub matching_rules: HashMap<DocPath, RuleList>,
    if let Some(plugin_configuration) = &self.plugin_configuration {
      table.set("plugin_configuration", lua.to_value(plugin_configuration)?)?;
    }
    table.into_lua(lua)
  }
}

fn body_to_lua<'lua>(lua: &'lua Lua, body: &OptionalBody) -> mlua::Result<Value<'lua>> {
  match body {
    OptionalBody::Missing => "Body::Missing".into_lua(lua),
    OptionalBody::Empty => "Body::Empty".into_lua(lua),
    OptionalBody::Null => "Body::Null".into_lua(lua),
    OptionalBody::Present(contents, content_type, hint) => {
      let table = lua.create_table()?;
      table.set("contents", contents.into_lua(lua)?)?;
      if let Some(content_type) = content_type {
        table.set("content_type", content_type.to_string().into_lua(lua)?)?;
      }
      if let Some(hint) = hint {
        table.set("content_type_hint", lua.to_value(hint)?)?;
      }
      table.into_lua(lua)
    }
  }
}

impl<'lua> FromLua<'lua> for CompareContentResult {
  fn from_lua(value: Value<'lua>, lua: &'lua Lua) -> mlua::Result<Self> {
    match &value {
      Value::Table(t) => if t.contains_key("error")? {
        Ok(CompareContentResult::Error(t.get("error")?))
      } else if t.contains_key("type-mismatch")? {
        let expected = String::from_lua(t.get("expected")?, lua)?;
        let actual = String::from_lua(t.get("actual")?, lua)?;
        Ok(CompareContentResult::TypeMismatch(expected, actual))
      } else if t.contains_key("mismatches")? {
        let mismatches = t.get::<_, Table>("mismatches")?;
        if mismatches.is_empty() {
          Ok(CompareContentResult::OK)
        } else {
          let mut result = hashmap!{};
          for entry in mismatches.pairs::<String, Value>() {
            let (k, v) = entry?;
            let mut mismatches = vec![];
            if let Ok(arr) = lua.from_value::<Vec<String>>(v.clone()) {
              for m in arr {
                mismatches.push(ContentMismatch {
                  expected: "".to_string(),
                  actual: "".to_string(),
                  mismatch: m,
                  path: k.clone(),
                  diff: None,
                  mismatch_type: None,
                })
              }
            } else {
              match &v {
                Value::Table(mismatch) => if !mismatch.is_empty() {
                  let mut content_mismatch = ContentMismatch::default();
                  if let Ok(expected) = mismatch.get::<_, String>("expected") {
                    content_mismatch.expected = expected;
                  }
                  if let Ok(actual) = mismatch.get::<_, String>("actual") {
                    content_mismatch.actual = actual;
                  }
                  if let Ok(mismatch) = mismatch.get::<_, String>("mismatch") {
                    content_mismatch.mismatch = mismatch;
                  }
                  if let Ok(path) = mismatch.get::<_, String>("path") {
                    content_mismatch.path = path;
                  }
                  if let Ok(diff) = mismatch.get::<_, String>("diff") {
                    content_mismatch.diff = Some(diff);
                  }
                  if let Ok(mismatch_type) = mismatch.get::<_, String>("mismatch_type") {
                    content_mismatch.mismatch_type = Some(mismatch_type);
                  }
                  mismatches.push(content_mismatch);
                }
                _ => {
                  mismatches.push(ContentMismatch {
                    expected: "".to_string(),
                    actual: "".to_string(),
                    mismatch: String::from_lua(v, lua)?,
                    path: k.clone(),
                    diff: None,
                    mismatch_type: None,
                  })
                }
              }
            }
            result.insert(k, mismatches);
          }
          Ok(CompareContentResult::Mismatches(result))
        }
      } else {
        Ok(CompareContentResult::OK)
      }
      _ => Err(mlua::Error::external(anyhow!("Invalid type for CompareContentResult, a table is required")))
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

    let rsa_validate_fn = runtime.create_function(move |_, (token, algorithm, key): (Vec<String>, String, String)| {
      debug!("Decoding public key as PKCS8 format");
      let public_key = RsaPublicKey::from_public_key_pem(key.as_str())
        .map_err(|err| mlua::Error::external(anyhow!("Failed to decode the public RSA key: {}", err)))?;
      let verifying_key = VerifyingKey::<Sha512>::new(public_key);
      let data = format!("{}.{}", token[0], token[1]);
      let decoded_signature = BASE64.decode(token[2].as_bytes())
        .or_else(|_| BASE64_NO_PAD.decode(token[2].as_bytes()))
        .map_err(|err| mlua::Error::external(anyhow!("Failed to base64 decode the token signature: {}", err)))?;
      let signature = Signature::try_from(decoded_signature.as_slice())
        .map_err(|err| mlua::Error::external(anyhow!("Failed to decode the token signature: {}", err)))?;
      match verifying_key.verify(data.as_bytes(), &signature) {
        Ok(_) => Ok(true),
        Err(err) => {
          error!("Signature verification failed: {}", err);
          Ok(false)
        }
      }
    })?;
    globals.set("rsa_validate", rsa_validate_fn)?;

    let b64_decode_fn = runtime.create_function(move |_, data: String| {
      debug!(%data, "Base64 decoding data");
      BASE64.decode(data.as_bytes())
        .or_else(|_| BASE64_NO_PAD.decode(data.as_bytes()))
        .map_err(|err| mlua::Error::external(anyhow!("Failed to decode base64 data: {}", err)))
    })?;
    globals.set("b64_decode_no_pad", b64_decode_fn)?;

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

    debug!("Got the following entries from the plugin: {:?}", result);
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
    trace!(?request, ">>> match_contents");
    let runtime = self.runtime.lock().unwrap();
    let globals = runtime.globals();
    let match_contents_fn: Function = globals.get("match_contents")?;
    match_contents_fn.call::<_, CompareContentResult>(request)
      .map_err(|err| anyhow!("Failed to execute Lua configure_interaction function: {}", err))
  }

  async fn configure_interaction(
    &self,
    content_type: &ContentType,
    definition: &HashMap<String, serde_json::Value>
  ) -> anyhow::Result<(Vec<InteractionContents>, Option<PluginConfiguration>)> {
    let runtime = self.runtime.lock().unwrap();
    let globals = runtime.globals();
    let table = runtime.create_table()?;
    for (key, value) in definition {
      table.set(key.as_str(), to_lua_value(value, &runtime)?)?;
    }
    let configure_interaction_fn: Function = globals.get("configure_interaction")?;
    configure_interaction_fn.call::<_, ConfigureInteractionResult>((content_type.to_string().as_str(), table))
      .map(|result| (result.contents, result.plugin_config))
      .map_err(|err| anyhow!("Failed to execute Lua configure_interaction function: {}", err))
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
