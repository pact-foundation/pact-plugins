Matching Rule Definition Expressions
------------------------------------

Test expectations (requests, responses or messages) setup via plugins use matching rule expressions to configure the
rules and values required for the test. Each definition can contain a number of expressions, separated by a comma.
Whitespace is ignored.

## Primitive values

Primitive (or scalar) values can be strings (quoted with single quotes), numbers, booleans or null.

## Expressions

The main types of expressions are one of the following:

### matching(TYPE [, CONFIG], EXAMPLE) or matching($'NAME')

Expression that defines a matching rule. Each matching rule requires the type of matching rule, and can contain an optional
configuration value. The final value is the example value to use.

Supported matching rules:

| Rule        | Description                                                                                           | Config Value       | Example                                                                       |
|-------------|-------------------------------------------------------------------------------------------------------|--------------------|-------------------------------------------------------------------------------|
| equalTo     | Value must be equal to the example                                                                    |                    | `matching(equalTo, 'Example value')`                                          |                
| type        | Value must be the same type as the example                                                            |                    | `matching(type, 'Example value')`                                             |    
| number      | Value must be a numeric value                                                                         |                    | `matching(number, 100.09)`                                                    |                  
| integer     | Value must be an integer value (no decimals)                                                          |                    | `matching(integer, 100)`                                                      |         
| decimal     | Value must be a decimal number (must have at least one significant figure after the decimal point)    |                    | `matching(decimnal, 100.01)`                                                  |         
| datetime    | Value must match a date-time format string                                                            | Format String      | `matching(datetime, 'yyyy-MM-dd HH:mm:ssZZZZZ', '2020-05-21 16:44:32+10:00')` |
| date        | Value must match a date format string                                                                 | Format String      | `matching(date, 'yyyy-MM-dd', '22:04')`                                       |
| time        | Value must match a time format string                                                                 | Format String      | `matching(time, 'HH:mm', '22:04')`                                            |
| regex       | Value must match a regular expression                                                                 | Regular expression | `matching(regex, '\\w{3}\\d+', 'abc123')`                                     |
| include     | Value must include the example value as a substring                                                   |                    | `matching(include, 'testing')`                                                |
| boolean     | Value must be a boolean                                                                               |                    | `matching(boolean, true)`                                                     |
| server      | Value must match the semver specification                                                             |                    | `matching(semver, '1.0.0')`                                                   |
| contentType | Value must be of the provided content type. This will preform a magic test on the bytes of the value. | Content type       | `matching(contentType, 'application/xml', '<?xml?><test/>')`                  |

The final form is a reference to another key. This is used to setup type matching using an example value, and is normally
used for collections. The name of the key must be a string value in single quotes. 

For example, to configure a type matcher where each value in a list must match the definition of a person:

```json
{
  "pact:match": "eachValue(matching($'person'))",
  "person": {
    "name": "Fred",
    "age": 100
  }
}
```

### notEmpty(EXAMPLE)

Expression that defines the value the same type as the example, must be present and not empty. This is used to defined 
required fields.

Example: `notEmpty('test')`

### eachKey(EXPRESSION)

Configures a matching rule to be applied to each key in a map.

For example: `eachKey(matching(regex, '\$(\.\w+)+', '$.test.one'))`

### eachValue(EXPRESSION)

Configures a matching rule to be applied to each value in a map or list.

For example: `eachValue(matching(type, 100))`  

## Grammar

There is a grammar for the definitions in [ANTLR4 format](./matching-rule-definition.g4).
