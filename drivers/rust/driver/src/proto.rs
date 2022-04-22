use pact_models::prelude::OptionalBody;
use pact_models::content_types::ContentTypeHint;

tonic::include_proto!("io.pact.plugin");

impl From<&OptionalBody> for Body {
  fn from(body: &OptionalBody) -> Self {
    match body {
      OptionalBody::Present(bytes, ct, ct_hint) => Body {
        content_type: ct.as_ref().map(|ct| ct.to_string()).unwrap_or_default(),
        content: Some(bytes.to_vec()),
        content_type_hint: match ct_hint {
          Some(ct_hint) => match ct_hint {
            ContentTypeHint::BINARY => body::ContentTypeHint::Binary as i32,
            ContentTypeHint::TEXT => body::ContentTypeHint::Text as i32,
            ContentTypeHint::DEFAULT => body::ContentTypeHint::Default as i32
          }
          None => body::ContentTypeHint::Default as i32
        }
      },
      _ => Body {
        content_type: "".to_string(),
        content: None,
        content_type_hint: body::ContentTypeHint::Default as i32
      }
    }
  }
}
