use anyhow::anyhow;
use logos::Logos;
use pact_models::matchingrules::expressions::{MatchingRuleDefinition, parse_matcher_def};
use prost_types::value::Kind;
use either::Either;
use either::Either::{Left, Right};

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
enum FieldToken {
  #[token("column")]
  Column,

  #[token(":")]
  Colon,

  #[regex("[a-zA-Z]+")]
  Text,

  #[regex("[0-9]+", |lex| lex.slice().parse().ok())]
  Int(i64)
}

// field -> "column" : int | text
pub(crate) fn parse_field(s: &str) -> anyhow::Result<Either<usize, String>> {
  let mut lex = FieldToken::lexer(s);
  let first = lex.next();
  if first == Some(Ok(FieldToken::Column)) {
    let second = lex.next();
    if second == Some(Ok(FieldToken::Colon)) {
      let third = lex.next();
      if let Some(Ok(FieldToken::Int(i))) = third {
        if i < 1 {
          Err(anyhow!("'{}' is not a valid field definition, expected an integer >= 1, got {}", s, i))
        } else {
          Ok(Left(i as usize))
        }
      } else if let Some(Ok(FieldToken::Text)) = third {
        Ok(Right(lex.slice().to_string()))
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

pub(crate) fn parse_value(v: &prost_types::Value) -> anyhow::Result<MatchingRuleDefinition> {
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
