package io.pact.example.csv.provider

import au.com.dius.pact.provider.junit5.HttpTestTarget
import au.com.dius.pact.provider.junit5.PactVerificationContext
import au.com.dius.pact.provider.junit5.PactVerificationInvocationContextProvider
import au.com.dius.pact.provider.junitsupport.Provider
import au.com.dius.pact.provider.junitsupport.loader.PactFolder
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.TestTemplate
import org.junit.jupiter.api.extension.ExtendWith
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort

@Provider("CsvServer")
@PactFolder("pacts")
@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
class CsvControllerPactTest {
  @LocalServerPort
  var port: Int = 0

  @TestTemplate
  @ExtendWith(PactVerificationInvocationContextProvider::class)
  fun testTemplate(context: PactVerificationContext) {
    context.verifyInteraction()
  }

  @BeforeEach
  fun setupTest(context: PactVerificationContext) {
    context.target = HttpTestTarget("localhost", port)
  }
}
