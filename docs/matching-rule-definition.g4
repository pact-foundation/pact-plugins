grammar MatchingRuleDefinition;

/**
* Parse a matcher expression into a MatchingRuleDefinition containing the example value, matching rules and any generator.
* The following are examples of matching rule definitions:
* * `matching(type,'Name')` - type matcher
* * `matching(number,100)` - number matcher
* * `matching(datetime, 'yyyy-MM-dd','2000-01-01')` - datetime matcher with format string
**/
matchingDefinition :
    matchingDefinitionExp ( COMMA matchingDefinitionExp  )* EOF
    ;

matchingDefinitionExp :
    (
      'matching' LEFT_BRACKET matchingRule RIGHT_BRACKET
      | 'notEmpty' LEFT_BRACKET primitiveValue RIGHT_BRACKET
      | 'eachKey' LEFT_BRACKET matchingDefinitionExp RIGHT_BRACKET
      | 'eachValue' LEFT_BRACKET matchingDefinitionExp RIGHT_BRACKET
    )
    ;

matchingRule :
  (
    ( 'equalTo' | 'type' ) COMMA primitiveValue )
  | 'number' COMMA ( DECIMAL_LITERAL | INTEGER_LITERAL )
  | 'integer' COMMA INTEGER_LITERAL
  | 'decimal' COMMA DECIMAL_LITERAL
  | matcherType=( 'datetime' | 'date' | 'time' ) COMMA string COMMA string
  | 'regex' COMMA string COMMA string
  | 'include' COMMA string
  | 'boolean' COMMA BOOLEAN_LITERAL
  | 'semver' COMMA string
  | 'contentType' COMMA string COMMA string
  | DOLLAR string
  ;

primitiveValue :
  string
  | DECIMAL_LITERAL
  | INTEGER_LITERAL
  | BOOLEAN_LITERAL
  ;

string :
  STRING_LITERAL
  | 'null'
  ;

INTEGER_LITERAL : '-'? DIGIT+ ;
DECIMAL_LITERAL : '-'? DIGIT+ '.' DIGIT+ ;
fragment DIGIT  : [0-9] ;

LEFT_BRACKET    : '(' ;
RIGHT_BRACKET   : ')' ;
STRING_LITERAL  : '\'' (~['])* '\'' ;
BOOLEAN_LITERAL : 'true' | 'false' ;
COMMA           : ',' ;
DOLLAR          : '$';

WS : [ \t\n\r] + -> skip ;
