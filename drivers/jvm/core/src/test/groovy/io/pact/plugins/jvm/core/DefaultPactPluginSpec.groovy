package io.pact.plugins.jvm.core

import io.grpc.ManagedChannel
import spock.lang.Specification

import java.util.concurrent.TimeUnit

class DefaultPactPluginSpec extends Specification {

  def 'on shutdown destroys the child process'() {
    given:
    def childProcess = Mock(ChildProcess)
    def manifest = new DefaultPactPluginManifest(
      'drivers/jvm/core/src/test/resources/plugins' as File,
      1,
      'TestPlugin',
      '1.2.3',
      'exec',
      null,
      'exec',
      [:],
      []
    )
    def channel = Mock(ManagedChannel)
    def plugin = new DefaultPactPlugin(
      childProcess,
      manifest,
      null,
      '1234',
      null,
      null,
      null
    )

    when:
    plugin.shutdown()

    then:
    1 * childProcess.destroy()
  }

  def 'on shutdown, shuts the GRPC channel down'() {
    given:
    def childProcess = Mock(ChildProcess)
    def manifest = new DefaultPactPluginManifest(
      'drivers/jvm/core/src/test/resources/plugins' as File,
      1,
      'TestPlugin',
      '1.2.3',
      'exec',
      null,
      'exec',
      [:],
      []
    )
    def channel = Mock(ManagedChannel)
    def plugin = new DefaultPactPlugin(
      childProcess,
      manifest,
      null,
      '1234',
      null,
      null,
      channel
    )

    when:
    plugin.shutdown()

    then:
    1 * channel.shutdownNow() >> channel
    1 * channel.awaitTermination(1, TimeUnit.SECONDS)
  }
}
