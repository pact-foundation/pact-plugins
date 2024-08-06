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
      | 'atLeast' LEFT_BRACKET DIGIT+ RIGHT_BRACKET
      | 'atMost' LEFT_BRACKET DIGIT+ RIGHT_BRACKET
    )
    ;

matchingRule :
  (
    ( 'equalTo' | 'type' ) COMMA primitiveValue )
  | 'number' COMMA ( DECIMAL_LITERAL | INTEGER_LITERAL | 'fromProviderState' fromProviderState )
  | 'integer' COMMA ( INTEGER_LITERAL | 'fromProviderState' fromProviderState )
  | 'decimal' COMMA ( DECIMAL_LITERAL | 'fromProviderState' fromProviderState )
  | matcherType=( 'datetime' | 'date' | 'time' ) COMMA string COMMA ( string | 'fromProviderState' fromProviderState )
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
  | 'fromProviderState' fromProviderState
  ;

fromProviderState :
  LEFT_BRACKET string COMMA primitiveValue RIGHT_BRACKET
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
