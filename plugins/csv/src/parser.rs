use anyhow::anyhow;
use log::debug;
use logos::{Lexer, Logos};
use pact_models::generators::Generator;
use pact_models::matchingrules::MatchingRule;
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

#[derive(Logos, Debug, PartialEq)]
enum ValueToken {
  #[token("matching")]
  Matching,

  #[token("(")]
  LeftBracket,

  #[token(")")]
  RightBracket,

  #[token(",")]
  Comma,

  #[regex("'[^']*'")]
  String,

  #[regex("[a-zA-Z]+")]
  Id,

  #[regex("-?[0-9]+", |lex| lex.slice().parse())]
  Int(i64),

  #[regex(r"-?[0-9]\.[0-9]+")]
  Decimal,

  #[regex(r"true|false")]
  Boolean,

  #[error]
  #[regex(r"[ \t\n\f]+", logos::skip)]
  Error
}

// "matching(type,'Name')",
// "matching(number,100)",
// "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
pub(crate) fn parse_value_def(v: &str) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  let mut lex = ValueToken::lexer(v);
  let next = lex.next();
  debug!("First Token: {:?}", next);
  if let Some(token) = next {
    if token == ValueToken::Matching {
      let next = lex.next().ok_or(anyhow!("'{}' is not a valid value definition, expected '('", v))?;
      if next == ValueToken::LeftBracket {
        let result = parse_matching_def(&mut lex)?;
        let next = lex.next().ok_or(anyhow!("'{}' is not a valid value definition, expected ')'", v))?;
        if next == ValueToken::RightBracket {
          let next = lex.next();
          if next.is_none() {
            Ok(result)
          } else {
            Err(anyhow!("'{}' is not a valid value definition, got '{}' after the closing bracket", v, lex.remainder()))
          }
        } else {
          Err(anyhow!("'{}' is not a valid value definition, expected closing bracket, got '{}'", v, lex.slice()))
        }
      } else {
        Err(anyhow!("'{}' is not a valid value definition, expected '(', got '{}'", v, lex.remainder()))
      }
    } else {
      Ok((v.to_string(), None, None))
    }
  } else {
    Ok((v.to_string(), None, None))
  }
}

fn parse_matching_def(lex: &mut logos::Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a matcher type"))?;
  if next == ValueToken::Id {
    match lex.slice() {
      "equality" => parse_equality(lex),
      "regex" => parse_regex(lex),
      "type" => parse_type(lex),
      "datetime" => parse_datetime(lex),
      "date" => parse_date(lex),
      "time" => parse_time(lex),
      "include" => parse_include(lex),
      "number" => parse_number(lex),
      "integer" => parse_integer(lex),
      "decimal" => parse_decimal(lex),
      "boolean" => parse_boolean(lex),
      _ => Err(anyhow!("Not a valid matcher definition, expected the type of matcher, got '{}'", lex.slice()))
    }
  } else {
    Err(anyhow!("Not a valid matcher definition, expected the type of matcher, got '{}'", lex.slice()))
  }
}

fn parse_equality(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Equality), None))
}

fn parse_regex(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let regex = parse_string(lex)?;
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Regex(regex)), None))
}

fn parse_type(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Type), None))
}

fn parse_datetime(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let format = parse_string(lex)?;
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Timestamp(format.clone())), Some(Generator::DateTime(Some(format.clone())))))
}

fn parse_date(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let format = parse_string(lex)?;
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Date(format.clone())), Some(Generator::Date(Some(format.clone())))))
}

fn parse_time(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let format = parse_string(lex)?;
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value, Some(MatchingRule::Time(format.clone())), Some(Generator::Time(Some(format.clone())))))
}

fn parse_include(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let value = parse_string(lex)?;
  Ok((value.clone(), Some(MatchingRule::Include(value.clone())), None))
}

fn parse_number(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a number"))?;
  if let ValueToken::Int(_) = next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Number), None))
  } else if ValueToken::Decimal == next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Number), None))
  } else {
    Err(anyhow!("Not a valid matcher definition, expected a number, got '{}'", lex.slice()))
  }
}

fn parse_integer(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected an integer"))?;
  if let ValueToken::Int(_) = next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Integer), None))
  } else {
    Err(anyhow!("Not a valid matcher definition, expected an integer, got '{}'", lex.slice()))
  }
}

fn parse_decimal(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a decimal number"))?;
  if let ValueToken::Int(_) = next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Decimal), None))
  } else if ValueToken::Decimal == next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Decimal), None))
  } else {
    Err(anyhow!("Not a valid matcher definition, expected a decimal number, got '{}'", lex.slice()))
  }
}

fn parse_boolean(lex: &mut Lexer<ValueToken>) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  parse_comma(lex)?;
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a boolean"))?;
  if ValueToken::Boolean == next {
    Ok((lex.slice().to_string(), Some(MatchingRule::Boolean), None))
  } else {
    Err(anyhow!("Not a valid matcher definition, expected a boolean, got '{}'", lex.slice()))
  }
}

fn parse_string(lex: &mut logos::Lexer<ValueToken>) -> anyhow::Result<String> {
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a starting quote"))?;
  if next == ValueToken::String {
    Ok(lex.slice().trim_matches('\'').to_string())
  } else {
    Err(anyhow!("Not a valid matcher definition, expected a starting quote, got '{}'", lex.slice()))
  }
}

fn parse_comma(lex: &mut Lexer<ValueToken>) -> anyhow::Result<()> {
  let next = lex.next()
    .ok_or(anyhow!("Not a valid matcher definition, expected a ','"))?;
  if next == ValueToken::Comma {
    Ok(())
  } else {
    Err(anyhow!("Not a valid matcher definition, expected a comma, got '{}'", lex.slice()))
  }
}

pub(crate) fn parse_value(v: &prost_types::Value) -> anyhow::Result<(String, Option<MatchingRule>, Option<Generator>)> {
  if let Some(kind) = &v.kind {
    match kind {
      Kind::StringValue(s) => parse_value_def(&s),
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
