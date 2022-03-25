package io.pact.plugins.jvm.core

import spock.lang.Specification
import spock.lang.Unroll

class DefaultPluginManagerSpec extends Specification {

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
}
