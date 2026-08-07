package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.matchingrules.RegexMatcher
import au.com.dius.pact.core.support.json.JsonValue
import io.pact.plugin.v2.PluginV2
import spock.lang.Specification

class FieldSpec extends Specification {
  private static final String PATH = '$.card.number'

  private static void registerCoreEntry(String key, CatalogueEntryType type) {
    CatalogueManager.INSTANCE.registerCoreEntries([
      new CatalogueEntry(type, CatalogueEntryProviderType.CORE, 'core', key)
    ])
  }

  private static FieldContext context() {
    new FieldContext(PATH, 'body', null, [:])
  }

  /**
   * The driver forwards whatever rule the host hands it - the rule's name and attributes - so any
   * rule exercises the plumbing. Once the Pact-JVM model grows a carrier for plugin-provided rules
   * (proposal 006 section 4), a plugin's own rule name arrives here by exactly this path.
   */
  private static aRule() {
    new RegexMatcher('\\d{16}')
  }

  def 'each scalar type crosses the boundary under its own arm'() {
    expect:
    value.toProto().valueCase == arm

    where:
    value                                          || arm
    FieldValue.Null.INSTANCE                       || PluginV2.FieldValue.ValueCase.NULLVALUE
    new FieldValue.Bool(true)                      || PluginV2.FieldValue.ValueCase.BOOLEANVALUE
    new FieldValue.Text('4111111111111111')        || PluginV2.FieldValue.ValueCase.STRINGVALUE
    new FieldValue.Integer(100L)                   || PluginV2.FieldValue.ValueCase.INTEGERVALUE
    new FieldValue.Decimal(100.5d)                 || PluginV2.FieldValue.ValueCase.DECIMALVALUE
    new FieldValue.Binary([0, -97, -110, -106] as byte[]) || PluginV2.FieldValue.ValueCase.BINARYVALUE
  }

  def 'field values round trip through the proto form'() {
    expect:
    FieldValue.fromProto(value.toProto()) == value

    where:
    value << [
      FieldValue.Null.INSTANCE,
      new FieldValue.Bool(true),
      new FieldValue.Text('4111111111111111'),
      new FieldValue.Integer(100L),
      new FieldValue.Decimal(-100.5d),
      new FieldValue.Binary([0, -97, -110, -106] as byte[])
    ]
  }

  def 'a whole number stays whole and a decimal stays decimal'() {
    // The distinction the integer, decimal and type rules are built on, and the reason FieldValue
    // does not put every value through a google.protobuf.Value
    expect:
    FieldValue.fromProto(new FieldValue.Integer(100L).toProto()) instanceof FieldValue.Integer
    FieldValue.fromProto(new FieldValue.Decimal(100.0d).toProto()) instanceof FieldValue.Decimal
  }

  def 'an unset proto value reads as null'() {
    expect:
    FieldValue.fromProto(PluginV2.FieldValue.newBuilder().build()) == FieldValue.Null.INSTANCE
  }

  def 'converts a Pact JSON value keeping whole numbers whole'() {
    expect:
    FieldValue.fromJson(new JsonValue.Integer('100'.chars)) instanceof FieldValue.Integer
    FieldValue.fromJson(new JsonValue.Decimal('100.5'.chars)) instanceof FieldValue.Decimal
    FieldValue.fromJson(new JsonValue.StringValue('visa'.chars)) instanceof FieldValue.Text
    FieldValue.fromJson(JsonValue.True.INSTANCE) == new FieldValue.Bool(true)
    FieldValue.fromJson(JsonValue.Null.INSTANCE) == FieldValue.Null.INSTANCE
  }

  /**
   * A plugin correlates what it logs with the test that caused it via `testRunId` in the request's
   * test context (proposal 008). The host has nothing to put there at the point a rule is applied,
   * so the driver fills it in.
   */
  def 'a field request carries the current test run id'() {
    given:
    def key = 'a-field-request-carries-the-current-test-run-id'
    def seen = []
    registerCoreEntry(key, CatalogueEntryType.MATCHER)
    CoreCapabilityRegistry.INSTANCE.registerFieldMatcher(key, { request ->
      seen << (request.hasTestContext() ? request.testContext.fieldsMap['testRunId']?.stringValue : null)
      PluginV2.MatchFieldResponse.newBuilder().build()
    } as CoreFieldMatcher)
    def matcher = FieldKt.findFieldMatcher(key)

    when:
    TestContext.setTestRunId('test-run-1')
    matcher.matchField(aRule(), new FieldValue.Text('a'), new FieldValue.Text('a'), context())
    TestContext.setTestRunId(null)
    matcher.matchField(aRule(), new FieldValue.Text('a'), new FieldValue.Text('a'), context())

    then:
    seen == ['test-run-1', null]

    cleanup:
    TestContext.setTestRunId(null)
  }

  def 'matchField dispatches to a registered core handler'() {
    given:
    def key = 'matchField dispatches to a registered core handler'
    registerCoreEntry(key, CatalogueEntryType.MATCHER)
    def seen = null
    CoreCapabilityRegistry.INSTANCE.registerFieldMatcher(key, { request ->
      seen = request
      PluginV2.MatchFieldResponse.newBuilder().build()
    } as CoreFieldMatcher)

    when:
    def matcher = FieldKt.findFieldMatcher(key)
    def result = matcher.matchField(aRule(), new FieldValue.Text('4111111111111111'),
      new FieldValue.Text('4012888888881881'), context())

    then:
    matcher.isCore()
    result.isEmpty()
    // The request carried what the caller passed in
    seen.path == PATH
    seen.mismatchType == 'body'
    seen.rule.type == 'regex'
    seen.expected.stringValue == '4111111111111111'
    seen.actual.stringValue == '4012888888881881'

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterFieldMatcher(key)
  }

  def 'matchField reports mismatches against the requested path'() {
    given:
    def key = 'matchField reports mismatches against the requested path'
    registerCoreEntry(key, CatalogueEntryType.MATCHER)
    CoreCapabilityRegistry.INSTANCE.registerFieldMatcher(key, { request ->
      PluginV2.MatchFieldResponse.newBuilder()
        // Deliberately no path/mismatchType: the driver fills them in from the request
        .addMismatches(PluginV2.ContentMismatch.newBuilder().setMismatch('fails the Luhn check'))
        .build()
    } as CoreFieldMatcher)

    when:
    def result = FieldKt.findFieldMatcher(key).matchField(aRule(),
      new FieldValue.Text('4111111111111111'), new FieldValue.Text('4111111111111112'), context())

    then:
    result.size() == 1
    result[0].mismatch == 'fails the Luhn check'
    result[0].path == PATH
    result[0].type == 'body'

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterFieldMatcher(key)
  }

  def 'matchField turns a handler error into a mismatch'() {
    given:
    def key = 'matchField turns a handler error into a mismatch'
    registerCoreEntry(key, CatalogueEntryType.MATCHER)
    CoreCapabilityRegistry.INSTANCE.registerFieldMatcher(key, { request ->
      PluginV2.MatchFieldResponse.newBuilder()
        .setError("'amx' is not a brand this plugin knows about")
        .build()
    } as CoreFieldMatcher)

    when:
    def result = FieldKt.findFieldMatcher(key).matchField(aRule(),
      new FieldValue.Text('4111111111111111'), new FieldValue.Text('4111111111111111'), context())

    then:
    result.size() == 1
    result[0].mismatch == "'amx' is not a brand this plugin knows about"

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterFieldMatcher(key)
  }

  def 'matchField fails clearly when no core handler is registered'() {
    given:
    def key = 'matchField fails clearly when no core handler is registered'
    registerCoreEntry(key, CatalogueEntryType.MATCHER)

    when:
    def result = FieldKt.findFieldMatcher(key).matchField(aRule(), FieldValue.Null.INSTANCE,
      FieldValue.Null.INSTANCE, context())

    then:
    result.size() == 1
    result[0].mismatch.contains('No core capability handler registered')
  }

  def 'generateField dispatches to a registered core handler'() {
    given:
    def key = 'generateField dispatches to a registered core handler'
    registerCoreEntry(key, CatalogueEntryType.GENERATOR)
    def seen = null
    CoreCapabilityRegistry.INSTANCE.registerFieldGenerator(key, { request ->
      seen = request
      PluginV2.GenerateFieldResponse.newBuilder()
        .setValue(new FieldValue.Text('4012888888881881').toProto())
        .build()
    } as CoreFieldGenerator)

    when:
    def generator = FieldKt.findFieldGenerator(key)
    def result = generator.generateField(new au.com.dius.pact.core.model.generators.RandomStringGenerator(16),
      new FieldValue.Text('4111111111111111'), FieldTestMode.CONSUMER, context())

    then:
    generator.isCore()
    result == new FieldValue.Text('4012888888881881')
    seen.path == PATH
    seen.testMode == PluginV2.GenerateContentRequest.TestMode.Consumer

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterFieldGenerator(key)
  }

  def 'generateField reports a generator error'() {
    given:
    def key = 'generateField reports a generator error'
    registerCoreEntry(key, CatalogueEntryType.GENERATOR)
    CoreCapabilityRegistry.INSTANCE.registerFieldGenerator(key, { request ->
      PluginV2.GenerateFieldResponse.newBuilder().setError('no such brand').build()
    } as CoreFieldGenerator)

    when:
    FieldKt.findFieldGenerator(key).generateField(
      new au.com.dius.pact.core.model.generators.RandomStringGenerator(16),
      FieldValue.Null.INSTANCE, FieldTestMode.CONSUMER, context())

    then:
    def ex = thrown(PactFieldGenerationException)
    ex.error == 'no such brand'

    cleanup:
    CoreCapabilityRegistry.INSTANCE.deregisterFieldGenerator(key)
  }

  def 'finding a rule that is not registered says so'() {
    when:
    FieldKt.findFieldMatcher('finding a rule that is not registered says so')

    then:
    thrown(PactCatalogueEntryNotFoundException)
  }

  def 'finding a rule that is a generator says so'() {
    given:
    def key = 'finding a rule that is a generator says so'
    registerCoreEntry(key, CatalogueEntryType.GENERATOR)

    when:
    FieldKt.findFieldMatcher(key)

    then:
    def ex = thrown(PactCatalogueEntryTypeMismatchException)
    ex.actualType == CatalogueEntryType.GENERATOR
    ex.expectedType == CatalogueEntryType.MATCHER
  }
}
