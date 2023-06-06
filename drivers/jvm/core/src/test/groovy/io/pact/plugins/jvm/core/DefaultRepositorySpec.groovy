package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.Result
import org.apache.hc.client5.http.fluent.Content
import org.apache.hc.client5.http.fluent.Request
import org.apache.hc.client5.http.fluent.Response
import spock.lang.Specification

class DefaultRepositorySpec extends Specification {
  def 'loading default index'() {
    when:
    def index = new DefaultRepository().defaultIndex().unwrap()
    def protobuf = index.entries['protobuf']
    def lastVersion = protobuf.versions.last()

    then:
    index.indexVersion > 0
    index.formatVersion == 0
    !index.timestamp.empty
    index.entries.size() > 0
    protobuf.name == 'protobuf'
    !protobuf.latestVersion.empty
    !protobuf.versions.empty
    lastVersion.version ==~ "\\d+\\.\\d+\\.\\d+"
    lastVersion.source == new ManifestSource.GitHubRelease("https://github.com/pactflow/pact-protobuf-plugin/releases/tag/v-${lastVersion.version}")
  }

  def 'if loading from GitHub fails, falls back to the local cached copy'() {
    given:
    def mockRequest = Mock(Request) {
      execute() >> { throw new RuntimeException("Boom") }
    }
    Repository repository = Spy(DefaultRepository) {
      getUrl(_) >> mockRequest
    }

    when:
    repository.fetchRepositoryIndex()

    then:
    1 * repository.loadLocalIndex()
  }

  def 'if unable to load the local cached copy, returns the default index'() {
    given:
    def mockRequest = Mock(Request) {
      execute() >> { throw new RuntimeException("Boom") }
    }
    Repository repository = Spy(DefaultRepository) {
      getUrl(_) >> mockRequest
      loadLocalIndex() >> new Result.Err('Boom')
    }

    when:
    repository.fetchRepositoryIndex()

    then:
    1 * repository.defaultIndex()
  }

  def 'fetchIndexFromGithub - returns an error if the SHA is not correct'() {
    given:
    def mockContent = Mock(Content) {
      asString() >> "1234567890"
    }
    def mockResponse = Mock(Response) {
      returnContent() >> mockContent
    }
    def mockRequest = Mock(Request) {
      execute() >> mockResponse
    }
    Repository repository = Spy(DefaultRepository) {
      getUrl(_) >> mockRequest
    }

    when:
    def result = repository.fetchIndexFromGithub()

    then:
    result instanceof Result.Err
    result.errorValue() == "SHA256 digest from GitHub does not match: expected c775e7b757ede630cd0aa1113bd102661ab38829ca52a6422ab782862f268646 but got 1234567890"
  }
}
