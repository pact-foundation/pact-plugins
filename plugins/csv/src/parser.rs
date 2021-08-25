use anyhow::anyhow;
use logos::Logos;
use pact_models::generators::Generator;
use pact_models::matchingrules::MatchingRule;
use pact_models::matchingrules::expressions::parse_matcher_def;
use prost_types::value::Kind;

#[derive(Logos, Debug, PartialEq)]
enum FieldToken {
  #[token("column")]
  Column,

  #[token(":")]
  Colon,

  #[regex("[a-zA-Z]+")]
  Text,

  #[regex("[0-9]+", |lex| lex.slice().parse())]
  Int(i64),

  #[error]
  #[regex(r"[ \t\n\f]+", logos::skip)]
  Error,
}

// field -> "column" : int
pub(crate) fn parse_field(s: &str) -> anyhow::Result<usize> {
  let mut lex = FieldToken::lexer(s);
  let first = lex.next();
  if first == Some(FieldToken::Column) {
    let second = lex.next();
    if second == Some(FieldToken::Colon) {
      let third = lex.next();
      if let Some(FieldToken::Int(i)) = third {
        if i < 1 {
          Err(anyhow!("'{}' is not a valid field definition, expected an integer >= 1, got {}", s, i))
        } else {
          Ok(i as usize)
        }
      } else {
        Err(anyhow!("'{}' is not a valid field definition, expected an integer, got '{}'", s, lex.remainder()))
      }
    } else {
      Err(anyhow!("'{}' is not a valid field definition, expected ':', got '{}'", s, lex.remainder()))
    }
  } else {
    Err(anyhow!("'{}' is not a valid field definition, expected 'column', got '{}'", s, lex.remainder()))
  }
}

pub(crate) fn parse_value(v: &prost_types::Value) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  if let Some(kind) = &v.kind {
    match kind {
      Kind::StringValue(s) => parse_matcher_def(&s),
      Kind::NullValue(_) => Err(anyhow!("Null is not a valid value definition value")),
      Kind::NumberValue(_) => Err(anyhow!("Number is not a valid value definition value")),
      Kind::BoolValue(_) => Err(anyhow!("Bool is not a valid value definition value")),
      Kind::StructValue(_) => Err(anyhow!("Struct is not a valid value definition value")),
      Kind::ListValue(_) => Err(anyhow!("List is not a valid value definition value")),
    }
  } else {
    Err(anyhow!("Not a valid value definition (missing value)"))
  }
}
