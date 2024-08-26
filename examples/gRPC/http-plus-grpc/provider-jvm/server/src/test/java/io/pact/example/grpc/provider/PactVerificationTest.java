package io.pact.example.grpc.provider;

import area_calculator.AreaCalculator;
import au.com.dius.pact.provider.junit5.HttpTestTarget;
import au.com.dius.pact.provider.junit5.PactVerificationContext;
import au.com.dius.pact.provider.junit5.PactVerificationInvocationContextProvider;
import au.com.dius.pact.provider.junit5.PluginTestTarget;
import au.com.dius.pact.provider.junitsupport.Provider;
import au.com.dius.pact.provider.junitsupport.loader.PactFolder;
import com.github.tomakehurst.wiremock.WireMockServer;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.TestTemplate;
import org.junit.jupiter.api.extension.ExtendWith;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import ru.lanwen.wiremock.ext.WiremockResolver;
import ru.lanwen.wiremock.ext.WiremockUriResolver;

import java.net.MalformedURLException;
import java.net.URL;
import java.util.Map;

import static com.github.tomakehurst.wiremock.client.WireMock.aResponse;
import static com.github.tomakehurst.wiremock.client.WireMock.post;
import static com.github.tomakehurst.wiremock.client.WireMock.urlPathEqualTo;

@Provider("area-calculator-provider")
@PactFolder("pacts")
@ExtendWith({
  WiremockResolver.class,
  WiremockUriResolver.class
})
class PactVerificationTest {
  private static final Logger LOGGER = LoggerFactory.getLogger(PactVerificationTest.class);
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
  void setupTest(PactVerificationContext context,
                 @WiremockResolver.Wiremock WireMockServer httpServer,
                 @WiremockUriResolver.WiremockUri String uri) throws MalformedURLException {
    context.setTarget(new PluginTestTarget(
      Map.of(
        "host", "localhost",
        "port", server.serverPort(),
        "transport", "grpc"
      )
    ));

    context.addAdditionalTarget(HttpTestTarget.fromUrl(new URL(uri)));
    AreaCalculator.AreaResponse response = AreaCalculator.AreaResponse.newBuilder().addValue(20.0f).build();
    httpServer.stubFor(
      post(urlPathEqualTo("/Calculator/calculateOne"))
      .willReturn(aResponse()
        .withStatus(200)
        .withHeader("content-type", "application/protobuf; message=.area_calculator.AreaResponse")
        .withBody(response.toByteArray())
      )
    );
  }

  @TestTemplate
  @ExtendWith(PactVerificationInvocationContextProvider.class)
  void pactVerificationTestTemplate(PactVerificationContext context) {
    context.verifyInteraction();
  }
}
