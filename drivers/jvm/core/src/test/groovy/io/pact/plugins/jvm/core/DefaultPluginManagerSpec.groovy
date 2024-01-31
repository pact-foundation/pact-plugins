package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.Consumer
import au.com.dius.pact.core.model.ContentType
import au.com.dius.pact.core.model.ContentTypeHint
import au.com.dius.pact.core.model.OptionalBody
import au.com.dius.pact.core.model.Provider
import au.com.dius.pact.core.model.V4Interaction
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.support.Result
import groovy.json.JsonOutput
import groovy.json.JsonSlurper
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import org.mockito.ArgumentCaptor
import org.mockito.Mockito
import spock.lang.Specification
import spock.lang.Unroll
import spock.util.environment.RestoreSystemProperties

import static org.mockito.Mockito.doReturn

@SuppressWarnings('LineLength')
class DefaultPluginManagerSpec extends Specification {
  def cleanup() {
    DefaultPluginManager.INSTANCE.PLUGIN_MANIFEST_REGISTER.remove('test/1.2.3')
    DefaultPluginManager.INSTANCE.PLUGIN_MANIFEST_REGISTER.remove('test-plugin/1.2.3')
  }

  @Unroll
  def 'plugin version compatibility check'() {
    expect:
    DefaultPluginManager.INSTANCE.versionsCompatible(actualVersion, required) == result

    where:

    actualVersion | required || result
    "1.0.0"       | null     || true
    "1.0.0"       | "1.0.0"  || true
    "1.0.0"       | "1.0.1"  || false
    "1.0.4"       | "1.0.3"  || true
    "1.1.0"       | "1.0.3"  || true
    "2.1.0"       | "1.1.0"  || true
    "1.1.0"       | "2.0.0"  || false
    "0.1.0"       | "0.0.3"  || true
  }

  def 'when loading manifests - will return any previously loaded manifest from the internal cache'() {
    given:
    def manifest = Mock(PactPluginManifest)
    def manager = DefaultPluginManager.INSTANCE
    manager.PLUGIN_MANIFEST_REGISTER['test/1.2.3'] = manifest

    when:
    def result = manager.loadPluginManifest('test', '1.2.3')

    then:
    result instanceof Result.Ok
    result.value == manifest
  }

  def 'when loading manifests - will ignore any previously loaded manifest in the internal cache if the versions do not match'() {
    given:
    def manifest = Mock(PactPluginManifest)
    def manager = DefaultPluginManager.INSTANCE
    manager.PLUGIN_MANIFEST_REGISTER['test/1.2.3'] = manifest

    when:
    def result = manager.loadPluginManifest('test', '1.2.4')

    then:
    result instanceof Result.Err
  }

  @RestoreSystemProperties
  def 'when loading manifests - will load the manifest from disk if it is not in the internal cache'() {
    given:
    def manager = DefaultPluginManager.INSTANCE
    def tempDir = File.createTempDir()
    System.setProperty('pact.plugin.dir', tempDir.toString())
    def manifestDir = new File(tempDir, 'test')
    manifestDir.mkdirs()
    def manifestFile = new File(manifestDir, 'pact-plugin.json')
    manifestFile.text = JsonOutput.toJson([
      name: 'test-plugin',
      version: '1.2.3',
      executableType: 'none',
      entryPoint: 'none'
    ])

    when:
    def result = manager.loadPluginManifest('test-plugin', '1.2.3')

    then:
    result instanceof Result.Ok
    result.value.name == 'test-plugin'
    result.value.version == '1.2.3'
    manager.PLUGIN_MANIFEST_REGISTER['test-plugin/1.2.3'] == result.value

    cleanup:
    tempDir.deleteDir()
  }

  @RestoreSystemProperties
  def 'when loading manifests - will accept the manifest of a different valid version'() {
    given:
    def manager = DefaultPluginManager.INSTANCE
    def tempDir = File.createTempDir()
    System.setProperty('pact.plugin.dir', tempDir.toString())
    def manifestDir = new File(tempDir, 'test')
    manifestDir.mkdirs()
    def manifestFile = new File(manifestDir, 'pact-plugin.json')

    def version = new Random().nextInt(100) + 2
    manifestFile.text = JsonOutput.toJson([
      name: 'test-plugin',
      version: "1.$version.99",
      executableType: 'none',
      entryPoint: 'none'
    ])

    when:
    def result = manager.loadPluginManifest('test-plugin', "1.2.$version")

    then:
    result instanceof Result.Ok
    result.value.name == 'test-plugin'
    result.value.version == "1.$version.99"
    manager.PLUGIN_MANIFEST_REGISTER["test-plugin/1.$version.99"] == result.value

    cleanup:
    tempDir.deleteDir()
  }

  @RestoreSystemProperties
  def 'when loading manifests - will accept the manifest of the maximum valid version found'() {
    given:
    def manager = DefaultPluginManager.INSTANCE
    def tempDir = File.createTempDir()
    System.setProperty('pact.plugin.dir', tempDir.toString())

    def manifestDir1 = new File(tempDir, 'test1')
    manifestDir1.mkdirs()
    def manifestFile1 = new File(manifestDir1, 'pact-plugin.json')
    manifestFile1.text = JsonOutput.toJson([
      name: 'test-plugin',
      version: "1.0.1",
      executableType: 'none',
      entryPoint: 'none'
    ])

    def manifestDir2 = new File(tempDir, 'test2')
    manifestDir2.mkdirs()
    def manifestFile2 = new File(manifestDir2, 'pact-plugin.json')
    manifestFile2.text = JsonOutput.toJson([
      name: 'test-plugin',
      version: "1.3.6",
      executableType: 'none',
      entryPoint: 'none'
    ])

    def manifestDir3 = new File(tempDir, 'test3')
    manifestDir3.mkdirs()
    def manifestFile3 = new File(manifestDir3, 'pact-plugin.json')

    def version = 500 + new Random().nextInt(100)
    manifestFile3.text = JsonOutput.toJson([
      name: 'test-plugin',
      version: "1.$version.0",
      executableType: 'none',
      entryPoint: 'none'
    ])

    when:
    def result = manager.loadPluginManifest('test-plugin', '1.0.0')

    then:
    result instanceof Result.Ok
    result.value.name == 'test-plugin'
    result.value.version == "1.$version.0"
    manager.PLUGIN_MANIFEST_REGISTER["test-plugin/1.$version.0"] == result.value

    cleanup:
    tempDir.deleteDir()
  }

  @Unroll
  def 'max version test'() {
    expect:
    DefaultPluginManager.INSTANCE.maxVersion(manifests(versions))?.version == expectedVersion

    where:

    versions                              | expectedVersion
    []                                    | null
    ['1.0.1']                             | '1.0.1'
    ['1.0.1', '1.0.2']                    | '1.0.2'
    ['1.0.3', '1.0.2']                    | '1.0.3'
    ['1.0.1', '1.0.7', '1.1.14', '1.1.7'] | '1.1.14'
  }

  List<PactPluginManifest> manifests(List versions) {
    versions.collect {
      new DefaultPactPluginManifest('/tmp' as File, 1, it, it, '', '', '', [:], [], [])
    }
  }

  def 'startMockServer - passes the mock server config on to the plugin'() {
    given:
    def manifest = Mock(PactPluginManifest) {
      getName() >> 'test-start-mockserver'
      getVersion() >> '1.2.3'
    }
    def manager = DefaultPluginManager.INSTANCE
    manager.PLUGIN_MANIFEST_REGISTER['test-start-mockserver/1.2.3'] = manifest
    CatalogueEntry entry = new CatalogueEntry(CatalogueEntryType.TRANSPORT, CatalogueEntryProviderType.PLUGIN,
      'test-start-mockserver', 'test')
    MockServerConfig config = new MockServerConfig('10.0.1.2', 11223, false)
    def pact = new V4Pact(new Consumer(), new Provider())
    PactPlugin mockPlugin = Mock() {
      getManifest() >> manifest
    }
    DefaultPluginManager.INSTANCE.PLUGIN_REGISTER['test-start-mockserver/1.2.3'] = mockPlugin
    def response = Plugin.StartMockServerResponse.newBuilder()
      .setDetails(Plugin.MockServerDetails.newBuilder().setKey('123abc').build())
      .build()

    def mockStub = Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
    ArgumentCaptor<Plugin.StartMockServerRequest> argument = ArgumentCaptor.forClass(Plugin.StartMockServerRequest)
    doReturn(response).when(mockStub).startMockServer(argument.capture())

    when:
    def result = manager.startMockServer(entry, config, pact)

    then:
    1 * mockPlugin.withGrpcStub(_) >> { args -> args[0].apply(mockStub) }
    result.key == '123abc'
    argument.value.hostInterface == '10.0.1.2'
    argument.value.port == 11223

    cleanup:
    DefaultPluginManager.INSTANCE.PLUGIN_REGISTER.remove('test-start-mockserver/1.2.3')
  }

  def 'invokeContentMatcher - must pass through the content type with the request'() {
    given:
    def manifest = Mock(PactPluginManifest) {
      getName() >> 'test-invokeContentMatcher'
      getVersion() >> '1.2.3'
    }
    def manager = DefaultPluginManager.INSTANCE
    PactPlugin mockPlugin = Mock() {
      getManifest() >> manifest
    }
    manager.PLUGIN_REGISTER['test-invokeContentMatcher/1.2.3'] = mockPlugin
    ContentMatcher matcher = new CatalogueContentMatcher(new CatalogueEntry(
      CatalogueEntryType.CONTENT_MATCHER, CatalogueEntryProviderType.PLUGIN, 'test-invokeContentMatcher', 'stuff'))
    OptionalBody expected = OptionalBody.body('{}', ContentType.fromString('application/stuff'))
    OptionalBody actual = OptionalBody.body('{}'.bytes, ContentType.fromString('application/x-stuff'),
      ContentTypeHint.BINARY)

    def response = Plugin.CompareContentsResponse.newBuilder().build()
    def mockStub = Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
    ArgumentCaptor<Plugin.CompareContentsRequest> argument = ArgumentCaptor.forClass(Plugin.CompareContentsRequest)
    doReturn(response).when(mockStub).compareContents(argument.capture())

    when:
    manager.invokeContentMatcher(matcher, expected, actual, false, [:], [:])

    then:
    1 * mockPlugin.withGrpcStub(_) >> { args -> args[0].apply(mockStub) }
    argument.value.actual.contentType == 'application/x-stuff'
    argument.value.actual.contentTypeHint == Plugin.Body.ContentTypeHint.BINARY
    argument.value.expected.contentType == 'application/stuff'
    argument.value.expected.contentTypeHint == Plugin.Body.ContentTypeHint.DEFAULT

    cleanup:
    DefaultPluginManager.INSTANCE.PLUGIN_REGISTER.remove('test-invokeContentMatcher/1.2.3')
  }

  def 'loadPlugin - if the requested plugin is not installed, but exists in the plugin index, it will auto-install it'() {
    given:
    def manager = DefaultPluginManager.INSTANCE
    manager.repository = Mock(Repository)
    manager.pluginDownloader = Mock(PluginDownloader)

    when:
    manager.loadPlugin('test', null)

    then:
    1 * manager.repository.fetchRepositoryIndex() >> new Result.Ok(
      new PluginRepositoryIndex(0, 0, '', [test: new PluginEntry('test', '1', [
        new PluginVersion('1', new ManifestSource.File('test'), null)
      ])]
    ))
    1 * manager.pluginDownloader.installPluginFromUrl('test') >> new Result.Err('boom')

    cleanup:
    manager.repository = new DefaultRepository()
    manager.pluginDownloader = DefaultPluginDownloader.INSTANCE
  }

  def 'prepare validation for interaction passes in pact with interaction keys set'() {
    given:
    def manifest = Mock(PactPluginManifest) {
      getName() >> 'test-prepareValidationForInteraction'
      getVersion() >> '1.2.3'
    }
    def manager = DefaultPluginManager.INSTANCE
    PactPlugin mockPlugin = Mock() {
      getManifest() >> manifest
    }
    manager.PLUGIN_REGISTER['test-prepareValidationForInteraction/1.2.3'] = mockPlugin
    def transportEntry = new CatalogueEntry(CatalogueEntryType.TRANSPORT, CatalogueEntryProviderType.PLUGIN,
      'test-prepareValidationForInteraction', 'stuff')

    def interaction = new V4Interaction.SynchronousHttp('test interaction for prepareValidationForInteraction')
    def pact = new V4Pact(new Consumer(), new Provider(), [ interaction ])

    def response = Plugin.VerificationPreparationResponse.newBuilder().build()
    def mockStub = Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
    ArgumentCaptor<Plugin.VerificationPreparationRequest> argument = ArgumentCaptor.forClass(Plugin.VerificationPreparationRequest)
    doReturn(response).when(mockStub).prepareInteractionForVerification(argument.capture())

    when:
    def result = manager.prepareValidationForInteraction(
        transportEntry,
        pact,
        interaction,
        [:]
    )
    def pactIn =  new JsonSlurper().parseText(argument.value.pact)
    def interactionIn = pactIn.interactions[0]

    then:
    1 * mockPlugin.withGrpcStub(_) >> { args -> args[0].apply(mockStub) }
    result instanceof Result.Ok
    interactionIn.key == argument.value.interactionKey

    cleanup:
    DefaultPluginManager.INSTANCE.PLUGIN_REGISTER.remove('test-prepareValidationForInteraction/1.2.3')
  }

  def 'verify interaction passes in pact with interaction keys set'() {
    given:
    def manifest = Mock(PactPluginManifest) {
      getName() >> 'test-verifyInteraction'
      getVersion() >> '1.2.3'
    }
    def manager = DefaultPluginManager.INSTANCE
    PactPlugin mockPlugin = Mock() {
      getManifest() >> manifest
    }
    manager.PLUGIN_REGISTER['test-verifyInteraction/1.2.3'] = mockPlugin
    def transportEntry = new CatalogueEntry(CatalogueEntryType.TRANSPORT, CatalogueEntryProviderType.PLUGIN,
      'test-verifyInteraction', 'stuff')

    def interaction = new V4Interaction.SynchronousHttp('test interaction for verifyInteraction')
    def pact = new V4Pact(new Consumer(), new Provider(), [ interaction ])

    def response = Plugin.VerifyInteractionResponse.newBuilder().build()
    def mockStub = Mockito.mock(PactPluginGrpc.PactPluginBlockingStub)
    ArgumentCaptor<Plugin.VerifyInteractionRequest> argument = ArgumentCaptor.forClass(Plugin.VerifyInteractionRequest)
    doReturn(response).when(mockStub).verifyInteraction(argument.capture())

    when:
    def result = manager.verifyInteraction(
      transportEntry,
      new InteractionVerificationData(OptionalBody.empty(), [:]),
      [:],
      pact,
      interaction
    )
    def pactIn =  new JsonSlurper().parseText(argument.value.pact)
    def interactionIn = pactIn.interactions[0]

    then:
    1 * mockPlugin.withGrpcStub(_) >> { args -> args[0].apply(mockStub) }
    result instanceof Result.Ok
    interactionIn.key == argument.value.interactionKey

    cleanup:
    DefaultPluginManager.INSTANCE.PLUGIN_REGISTER.remove('test-verifyInteraction/1.2.3')
  }
}
