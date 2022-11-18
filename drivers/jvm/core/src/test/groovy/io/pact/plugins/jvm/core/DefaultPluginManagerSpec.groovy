package io.pact.plugins.jvm.core

import au.com.dius.pact.core.model.Consumer
import au.com.dius.pact.core.model.Provider
import au.com.dius.pact.core.model.V4Pact
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import groovy.json.JsonOutput
import io.grpc.CallOptions
import io.grpc.Channel
import io.grpc.stub.AbstractStub
import io.pact.plugin.PactPluginGrpc
import io.pact.plugin.Plugin
import org.mockito.ArgumentCaptor
import org.mockito.Mockito
import spock.lang.Specification
import spock.lang.Unroll
import spock.util.environment.RestoreSystemProperties

import static org.mockito.Mockito.doReturn

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
    result instanceof Ok
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
    result instanceof Err
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
    result instanceof Ok
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
    result instanceof Ok
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
    result instanceof Ok
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
}
