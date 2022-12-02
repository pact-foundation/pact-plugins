package io.pact.example.grpc.maven;

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

/**
 * Pact verification test for the AreaCalculator calculate service method call. This test requires one of the Pact files
 * from the consumer tests to be copied into the src/test/resources/pacts directory.
 */
@Provider("area-calculator-provider")
@PactFolder("pacts")
class PactVerificationTest {
  static Server server;

  /**
   * Start the gRPC AreaCalculator server
   */
  @BeforeAll
  static void setup() {
    server = new Server();
    server.start();
  }

  /**
   * Shut the server down after the test
   */
  @AfterAll
  static void cleanup() {
    server.stop();
  }

  /**
   * Configure the test target to use the Protobuf plugin. This is done by setting the transport for the test to grpc.
   */
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

  /**
   * Get the Pact framework to execute the test for all interactions in the loaded Pact files
   */
  @TestTemplate
  @ExtendWith(PactVerificationInvocationContextProvider.class)
  void pactVerificationTestTemplate(PactVerificationContext context) {
    context.verifyInteraction();
  }
}
