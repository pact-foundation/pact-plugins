use crate::jsonrpc::JsonRpcInteractionConfig;

#[derive(Debug, Clone)]
pub struct PactInteraction {
  pub key: String,
  pub description: String,
  pub config: JsonRpcInteractionConfig,
}
