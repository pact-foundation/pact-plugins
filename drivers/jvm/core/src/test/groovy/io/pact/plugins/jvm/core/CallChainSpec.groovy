package io.pact.plugins.jvm.core

import spock.lang.Specification

import java.time.Duration

class CallChainSpec extends Specification {
  def 'pushCall succeeds for a new entry and pops on close'() {
    given:
    def chainId = 'pushCall succeeds for a new entry and pops on close'

    when:
    def guardA = CallChain.INSTANCE.pushCall(chainId, 'content-matcher/xml')
    CallChain.INSTANCE.pushCall(chainId, 'content-matcher/csv')

    then:
    noExceptionThrown()

    when:
    guardA.close()
    // both guards have closed, so the chain should be gone and the keys reusable
    CallChain.INSTANCE.pushCall(chainId, 'content-matcher/xml').close()

    then:
    noExceptionThrown()
  }

  def 'pushCall rejects a repeated entry key in the same chain'() {
    given:
    def chainId = 'pushCall rejects a repeated entry key in the same chain'
    CallChain.INSTANCE.pushCall(chainId, 'content-matcher/xml')

    when:
    CallChain.INSTANCE.pushCall(chainId, 'content-matcher/xml')

    then:
    def ex = thrown(PactCallChainCycleException)
    ex.entryKey == 'content-matcher/xml'
    ex.chain == ['content-matcher/xml']
  }

  def 'pushCall allows the same entry key in different chains'() {
    given:
    def key = 'content-matcher/xml'
    CallChain.INSTANCE.pushCall('chain-a-pushCall allows the same entry key in different chains', key)

    when:
    CallChain.INSTANCE.pushCall('chain-b-pushCall allows the same entry key in different chains', key)

    then:
    noExceptionThrown()
  }

  def 'deadline helpers compute expiry and remaining budget'() {
    given:
    def futureDeadline = CallChain.INSTANCE.deadlineFrom(Duration.ofSeconds(60))
    def pastDeadline = CallChain.INSTANCE.nowMs() - 1000

    expect:
    !CallChain.INSTANCE.isExpired(futureDeadline)
    CallChain.INSTANCE.remaining(futureDeadline).toMillis() > 0
    CallChain.INSTANCE.isExpired(pastDeadline)
    CallChain.INSTANCE.remaining(pastDeadline) == Duration.ZERO
  }
}
