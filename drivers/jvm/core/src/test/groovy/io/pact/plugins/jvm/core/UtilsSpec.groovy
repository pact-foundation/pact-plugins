package io.pact.plugins.jvm.core

import com.google.protobuf.ListValue
import com.google.protobuf.NullValue
import com.google.protobuf.Struct
import com.google.protobuf.Value
import spock.lang.Specification
import spock.lang.Unroll

@SuppressWarnings('LineLength')
class UtilsSpec extends Specification {
  @Unroll
  def 'converting Protobuf Struct to a map'() {
    expect:
    Utils.INSTANCE.structToMap(value) == result

    where:

    value       | result
    null        | [:]
    structValue() | [null: null, num: 786.0, str: 'adx0diuisd', bool: false, list: [786.0, null, 'adx0diuisd'], struct: [one: 786.0]]
  }

  def 'converting map to Protobuf Struct'() {
    expect:
    Utils.INSTANCE.mapToProtoStruct([
      null: null,
      num: 786.0,
      str: 'adx0diuisd',
      bool: false,
      list: [786.0, null, 'adx0diuisd'],
      struct: [one: 786.0]
    ]) == structValue()
  }

  def 'converting a map with POJO to Protobuf Struct'() {
    given:
    def struct = Struct.newBuilder()
      .putAllFields([
        name: Value.newBuilder().setStringValue('test').build(),
        version: Value.newBuilder().setStringValue('1.2.3').build(),
        type: Value.newBuilder().setStringValue('OSPackage').build()
      ])
      .build()
    def result = Struct.newBuilder()
      .putFields('value', Value.newBuilder().setStructValue(struct).build())
      .build()

    expect:
    Utils.INSTANCE.mapToProtoStruct([
      value: new PluginDependency('test', '1.2.3', PluginDependencyType.OSPackage)
    ]) == result
  }

  static Value nullValue() {
    Value.newBuilder().setNullValue(NullValue.NULL_VALUE).build()
  }

  static Struct structValue() {
    def struct = Struct.newBuilder()
      .putAllFields([one: numberValue()])
      .build()
    Struct.newBuilder()
      .putAllFields([
        null: nullValue(),
        num: numberValue(),
        str: stringValue(),
        bool: booleanValue(),
        list: listValue(),
        struct: Value.newBuilder().setStructValue(struct).build()
      ])
      .build()
  }

  static Value numberValue() {
    Value.newBuilder().setNumberValue(786.toDouble()).build()
  }

  static Value stringValue() {
    Value.newBuilder().setStringValue('adx0diuisd').build()
  }

  static Value booleanValue() {
    Value.newBuilder().setBoolValue(false).build()
  }

  static Value listValue() {
    def builder = ListValue.newBuilder()
    builder.addValues(numberValue())
    builder.addValues(nullValue())
    builder.addValues(stringValue())
    Value.newBuilder().setListValue(builder.build()).build()
  }
}
