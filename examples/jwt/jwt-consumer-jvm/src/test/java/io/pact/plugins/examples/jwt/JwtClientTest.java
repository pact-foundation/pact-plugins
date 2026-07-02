package io.pact.plugins.examples.jwt;

import au.com.dius.pact.consumer.MockServer;
import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import io.jsonwebtoken.Jwts;
import org.bouncycastle.openssl.PEMKeyPair;
import org.bouncycastle.openssl.PEMParser;
import org.bouncycastle.openssl.jcajce.JcaPEMKeyConverter;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.io.StringReader;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.security.PrivateKey;
import java.util.Date;
import java.util.Map;

import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.is;

@ExtendWith(PactConsumerTestExt.class)
class JwtClientTest {
  // Test-only RSA private key (PKCS#1), same one used by the jwt-consumer-rust example.
  private static final String PRIVATE_KEY = "-----BEGIN RSA PRIVATE KEY-----\n" +
    "MIIEowIBAAKCAQEAvenHRTv98Lg6FAGkCy35yhcpL+aVw6mYeipYowLGl3zyBfRt\n" +
    "XRlnKAUYPozrfB+/QLqZ+TQMSVamD0q3nYiCwla93IMWscO4MsaVMmljxfl84x7A\n" +
    "djms4hS7IMA9aXOlir0mCPPXE6R89d+pjmErWba+svY/wrl2WCTuqfRLa8bKcgmR\n" +
    "0++35AhzyY8Wxp+8JPBUVSvOH2do1NI5e9tUwFGBUtowKnwT32oC9iYAgo9PcTiv\n" +
    "28mV/FHbqQwRMKbhh0A4SUv2e01YtIuNhvpd3Z74nsdG4lw8VWGyFNbMTrasc1PG\n" +
    "iGReNYTp66S8s+bmivINloxsPrwrstZPE2UJOQIDAQABAoIBAATczXtaU+Ar92C3\n" +
    "wgl/PdwMx8MwNjlySDMojmhuE8OhMVkxrvMpSVje+IXxeb4N2gnAPV0CFiZyj4Ho\n" +
    "udbQvfhX3DifKp+WkUrLhtpplGJnRulRyj+8rk6DlV77TRc8HMr2mNi11ZXtKj3p\n" +
    "YiABIOkFItDWOT+1G/CZ0XqMhLnXq8sfV6Y77eV5ue9G/SeUQlKoW7MA0zth+hBo\n" +
    "ISRo1I8DrJFhJhWhO4OhMTBcV2HbEbbJ9GuD1FA44NJsZPf3DZoq/N0hj9/uopm4\n" +
    "dKVx6Dcr0AP8JN5jjq4CE4hdnz/nr889liwG1C6mElgfsU7Gw6gqKV2PNeO6n+NU\n" +
    "qtKSUnkCgYEA+Ss1DkAL1Rb/z9Ap6VpIpjL84fC/K5HsEjg3rEuEu1xto22MAMz7\n" +
    "rCDelxYXU/NYCeh6sCIQblFYc9hkmmzyJbrcq/yLDZ5HmSOs/RNV+hOTFFFi95VV\n" +
    "5X6OPIjFHzLgo3BjbYtEA+gtoEIMZ/XctfHvcPUssfr2aq6rc5r42+sCgYEAwx6v\n" +
    "eeDYk48mof2GrOD8yJvNQHL9iJXXQ/DJ6it14R5JO2iNbX3y9TDb2Xu+KQU6/66g\n" +
    "095M3JlmeyT8/eFMwH5978Ci2pmDEs+QZXG6GwFFEwRxTMdQoHDMgue8TMLm3FJd\n" +
    "D9FXPk9wKBGjGN3DB5G3AzHqVqaN+Xij9/aR2msCgYBSlwLIDWyenjf+zxYFVjq8\n" +
    "dCwkTCNhssWYKHAzuPhvDiz9PcNpRIirPl3poJXs6r0k051PIotltarnAzQdh70f\n" +
    "ynd4voXs5qj+1rdxT2ZxNOnMk0mFnUdSgYduAzuroraZFhiu57mMvfnZo+ruzqzw\n" +
    "1heyzmGZQQFKzUjhUd3pLwKBgGCKTDQ3ZbEMwQahVAMxhqETRWi//GWaDdpVxvGP\n" +
    "81EhFQbJ4j/sc0uRkxV2Pk45gkmDc5ugf9MeKzB+ypYq5TjQ3SrE207haZLjFAS9\n" +
    "UmGOLUkNh6l/bIsVhHq4gdhRDrywG895unrf/xQ0NchV4Otb03tHNTUOT2zBng9P\n" +
    "9jZlAoGBAIolo+I7P3pMo87uy5qDDmxQaCj9wsIzKbliTpDb3WvmHimpaCCGOgbi\n" +
    "Oz4QOdgkf+Unl1cOnF8EAQ0J2bp+Cck7kb8u3cjKY1AR17ugIksOaB9mGB0bJ7hu\n" +
    "tnS+LGbydGz22ZMCG6LF0Z+dNX0zZoWKsvGAWTJBVSANnTo95igh\n" +
    "-----END RSA PRIVATE KEY-----";

  @Pact(consumer = "JwtClient")
  V4Pact pact(PactBuilder builder) {
    return builder
      .usingPlugin("jwt")
      .expectsToReceive("request for a token exchange", "core/transport/http")
      .with(Map.of(
        "request.path", "/token",
        "request.method", "POST",
        "request.contents", Map.of(
          "pact:content-type", "application/jwt+json",
          "audience", "1234566778",
          "subject", "slksjkdjkdks",
          "issuer", "ldsdkdalds",
          "algorithm", "RS512",
          "key-id", "key-112345564",
          "private-key", PRIVATE_KEY
        ),
        "response.status", "200",
        "response.contents", Map.of(
          "pact:content-type", "application/jwt+json",
          "audience", "1234566778",
          "subject", "slksjkdjkdks",
          "issuer", "ldsdkdalds",
          "algorithm", "RS512",
          "key-id", "key-112345564",
          "private-key", PRIVATE_KEY
        )
      ))
      .toPact();
  }

  @Test
  @PactTestFor(providerName = "JwtServer", pactMethod = "pact")
  void postJwt(MockServer mockServer) throws Exception {
    PrivateKey privateKey = loadPrivateKey(PRIVATE_KEY);
    String token = Jwts.builder()
      .header().add("typ", "JWT").add("kid", "key-112345564").and()
      .claims(Map.of(
        "aud", "1234566778",
        "sub", "slksjkdjkdks",
        "iss", "ldsdkdalds"
      ))
      .expiration(new Date(System.currentTimeMillis() + 300_000))
      .signWith(privateKey, Jwts.SIG.RS512)
      .compact();

    HttpClient client = HttpClient.newHttpClient();
    HttpRequest request = HttpRequest.newBuilder()
      .uri(URI.create(mockServer.getUrl() + "/token"))
      .header("content-type", "application/jwt+json")
      .POST(HttpRequest.BodyPublishers.ofString(token))
      .build();
    HttpResponse<String> response = client.send(request, HttpResponse.BodyHandlers.ofString());
    assertThat(response.statusCode(), is(200));
  }

  // The JDK's KeyFactory only understands PKCS#8-encoded keys, not the PKCS#1 "RSA PRIVATE
  // KEY" PEM format the jwt plugin itself accepts - BouncyCastle parses PKCS#1 PEM directly.
  private static PrivateKey loadPrivateKey(String pem) throws Exception {
    Object parsed = new PEMParser(new StringReader(pem)).readObject();
    return new JcaPEMKeyConverter().getKeyPair((PEMKeyPair) parsed).getPrivate();
  }
}
