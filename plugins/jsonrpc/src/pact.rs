use anyhow::{anyhow, Context, Result};
use serde_json::Value;

use crate::jsonrpc::JsonRpcInteractionConfig;

#[derive(Debug, Clone)]
pub struct PactInteraction {
  pub key: String,
  pub description: String,
  pub config: JsonRpcInteractionConfig,
}

pub fn parse_plugin_interactions(
  pact_json: &str,
  plugin_name: &str,
) -> Result<Vec<PactInteraction>> {
  let pact: Value = serde_json::from_str(pact_json).context("failed to parse pact JSON")?;
  let interactions = pact
    .get("interactions")
    .and_then(Value::as_array)
    .context("pact JSON did not contain an interactions array")?;

  interactions
    .iter()
    .enumerate()
    .filter_map(|(index, interaction)| {
      extract_plugin_interaction(interaction, plugin_name, index).transpose()
    })
    .collect()
}

pub fn find_plugin_interaction(
  pact_json: &str,
  plugin_name: &str,
  interaction_key: &str,
) -> Result<PactInteraction> {
  let interactions = parse_plugin_interactions(pact_json, plugin_name)?;
  interactions
    .into_iter()
    .find(|interaction| interaction.key == interaction_key)
    .ok_or_else(|| anyhow!("did not find JSON-RPC interaction with key '{interaction_key}'"))
}

fn extract_plugin_interaction(
  interaction: &Value,
  plugin_name: &str,
  index: usize,
) -> Result<Option<PactInteraction>> {
  let Some(plugin_config) = interaction
    .get("pluginConfiguration")
    .and_then(|value| value.get(plugin_name))
    .map(|value| {
      value
        .get("interactionConfiguration")
        .cloned()
        .unwrap_or_else(|| value.clone())
    })
  else {
    return Ok(None);
  };

  let config = JsonRpcInteractionConfig::from_contents_config(plugin_config)?;
  let key = interaction
    .get("key")
    .or_else(|| interaction.get("uniqueKey"))
    .and_then(Value::as_str)
    .map(str::to_string)
    .unwrap_or_else(|| format!("interaction-{index}"));
  let description = interaction
    .get("description")
    .and_then(Value::as_str)
    .map(str::to_string)
    .unwrap_or_else(|| key.clone());

  Ok(Some(PactInteraction {
    key,
    description,
    config,
  }))
}
