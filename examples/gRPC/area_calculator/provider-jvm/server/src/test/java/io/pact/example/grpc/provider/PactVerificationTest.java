package io.pact.example.grpc.provider;

import au.com.dius.pact.provider.junit5.PactVerificationContext;
import au.com.dius.pact.provider.junit5.PactVerificationInvocationContextProvider;
import au.com.dius.pact.provider.junit5.PluginTestTarget;
import au.com.dius.pact.provider.junitsupport.Provider;
import au.com.dius.pact.provider.junitsupport.loader.PactFolder;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.TestTemplate;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.Map;

@Provider("area-calculator-provider")
@PactFolder("pacts")
class PactVerificationTest {
  static Server server;

  @BeforeAll
  static void setup() {
    server = new Server();
    server.start();
  }

  @AfterAll
  static void cleanup() {
    server.stop();
  }

  @BeforeEach
  void setupTest(PactVerificationContext context) {
    context.setTarget(new PluginTestTarget(
      Map.of(
        "host", "localhost",
        "port", server.serverPort(),
        "transport", "grpc"
      )
    ));
  }

  @TestTemplate
  @ExtendWith(PactVerificationInvocationContextProvider.class)
  void pactVerificationTestTemplate(PactVerificationContext context) {
    context.verifyInteraction();
  }
}
