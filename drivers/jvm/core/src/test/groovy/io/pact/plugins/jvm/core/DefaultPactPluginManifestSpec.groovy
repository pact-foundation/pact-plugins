package io.pact.plugins.jvm.core

import spock.lang.Specification

import javax.json.Json

class DefaultPactPluginManifestSpec extends Specification {

  def 'converting manifest to JSON'() {
    given:
    def manifest = new DefaultPactPluginManifest(
      'drivers/jvm/core/src/test/resources/plugins' as File,
      1,
      'TestPlugin',
      '1.2.3',
      'exec',
      null,
      'exec',
      []
    )

    expect:
    manifest.toMap() == [
      pluginDir: 'drivers/jvm/core/src/test/resources/plugins',
      pluginInterfaceVersion: 1,
      name: 'TestPlugin',
      version: '1.2.3',
      executableType: 'exec',
      entryPoint: 'exec'
    ]
  }

  def 'converting manifest to JSON - with min required version'() {
    given:
    def manifest = new DefaultPactPluginManifest(
      'drivers/jvm/core/src/test/resources/plugins' as File,
      1,
      'TestPlugin',
      '1.2.3',
      'ruby',
      '1.7.1',
      'exec.rb',
      []
    )

    expect:
    manifest.toMap() == [
      pluginDir: 'drivers/jvm/core/src/test/resources/plugins',
      pluginInterfaceVersion: 1,
      name: 'TestPlugin',
      version: '1.2.3',
      executableType: 'ruby',
      entryPoint: 'exec.rb',
      minimumRequiredVersion: '1.7.1'
    ]
  }

  def 'converting manifest to JSON - with dependencies'() {
    def file = 'drivers/jvm/core/src/test/resources/plugins' as File
    given:
    def manifest = new DefaultPactPluginManifest(
      file,
      1,
      'TestPlugin',
      '1.2.3',
      'ruby',
      '1.7.1',
      'exec.rb',
      [
        new PluginDependency('dep1', '1.0', PluginDependencyType.Plugin),
        new PluginDependency('dep2', '2.0', PluginDependencyType.Library)
      ]
    )

    expect:
    manifest.toMap() == [
      pluginDir: file.toString(),
      pluginInterfaceVersion: 1,
      name: 'TestPlugin',
      version: '1.2.3',
      executableType: 'ruby',
      entryPoint: 'exec.rb',
      minimumRequiredVersion: '1.7.1',
      dependencies: [
        [name: 'dep1', version: '1.0', type: 'Plugin'],
        [name: 'dep2', version: '2.0', type: 'Library']
      ]
    ]
  }

  def 'loading manifest from JSON'() {
    given:
    InputStream pluginFile = DefaultPactPluginManifestSpec.getResourceAsStream('/pact-plugin.json')
    def pluginJson = Json.createReader(pluginFile).readObject()

    when:
    def pluginManifest = DefaultPactPluginManifest.fromJson('pact-plugin.json' as File, pluginJson)

    then:
    pluginManifest == new DefaultPactPluginManifest(
      'pact-plugin.json' as File,
      1,
      'csv',
      '0.0.0',
      'exec',
      null,
      'pact-plugins/plugins/csv/target/debug/pact-plugin-csv',
      []
    )
  }
}
