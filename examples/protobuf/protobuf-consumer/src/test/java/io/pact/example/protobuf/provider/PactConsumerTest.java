package io.pact.example.protobuf.provider;

import au.com.dius.pact.consumer.dsl.PactBuilder;
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt;
import au.com.dius.pact.consumer.junit5.PactTestFor;
import au.com.dius.pact.consumer.junit5.ProviderType;
import au.com.dius.pact.core.model.PactSpecVersion;
import au.com.dius.pact.core.model.V4Interaction;
import au.com.dius.pact.core.model.V4Pact;
import au.com.dius.pact.core.model.annotations.Pact;
import com.google.protobuf.InvalidProtocolBufferException;
import io.pact.plugin.Plugin;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

import java.util.List;
import java.util.Map;
import java.util.Set;

import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static org.hamcrest.MatcherAssert.assertThat;
import static org.hamcrest.Matchers.equalTo;
import static org.hamcrest.Matchers.is;

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "protobuf-provider", providerType = ProviderType.ASYNCH, pactVersion = PactSpecVersion.V4)
class PactConsumerTest {
    @Pact(consumer = "protobuf-consumer")
    V4Pact initPluginMessage(PactBuilder builder) {
        return builder
          .usingPlugin("protobuf")
          .expectsToReceive("init plugin message", "core/interaction/message")
          .with(Map.of(
            "message.contents", Map.of(
              "pact:proto", filePath("../../../proto/plugin.proto"),
              "pact:message-type", "InitPluginRequest",
              "pact:content-type", "application/protobuf",
              "implementation", "notEmpty('pact-jvm-driver')",
              "version", "matching(semver, '0.0.0')"
            )
          ))
          .toPact();
    }

    @Test
    @PactTestFor(pactMethod = "initPluginMessage")
    void consumeInitPluginMessage(V4Interaction.AsynchronousMessage message) throws InvalidProtocolBufferException {
        Plugin.InitPluginRequest request = Plugin.InitPluginRequest.parseFrom(message.getContents().getContents().getValue());
        assertThat(request.getImplementation(), is("pact-jvm-driver"));
        assertThat(request.getVersion(), is("0.0.0"));
    }

    @Pact(consumer = "protobuf-consumer")
    V4Pact configureInteractionResponseMessage(PactBuilder builder) {
        return builder
          .usingPlugin("protobuf")
          .expectsToReceive("Configure Interaction Response", "core/interaction/message")
          .with(Map.of(
            "message.contents", Map.of(
              "pact:proto", filePath("../../../proto/plugin.proto"),
              "pact:message-type", "ConfigureInteractionResponse",
              "pact:content-type", "application/protobuf",
              "contents", Map.of(
                "contentType", "notEmpty('application/json')",
                "content", "matching(contentType, 'application/json', '{}')",
                "contentTypeHint", "matching(equalTo, 'TEXT')"
              ),
              "rules", Map.of(
                "pact:match", "eachKey(matching(regex, '$(\\.\\w+)+', '$.test.one'))",
                "$.test.one", Map.of(
                  "rules", Map.of(
                    "pact:match", "eachValue(matching($'items'))",
                    "items", Map.of(
                      "type", "notEmpty('regex')"
                    )
                  )
                )
              ),
              "generators", Map.of(
                "$.test.one", Map.of(
                  "type", "notEmpty('DateTime')",
                  "values", Map.of(
                    "format", "matching(equalTo, 'YYYY-MM-DD')"
                  )
                ),
                "$.test.two", Map.of(
                  "type", "notEmpty('DateTime')",
                  "values", Map.of(
                    "format", "matching(equalTo, 'YYYY-MM-DD')"
                  )
                )
              )
            )
          ))
          .toPact();
    }

    @Test
    @PactTestFor(pactMethod = "configureInteractionResponseMessage")
    void consumeConfigureInteractionResponseMessage(V4Interaction.AsynchronousMessage message) throws InvalidProtocolBufferException {
        Plugin.ConfigureInteractionResponse response = Plugin.ConfigureInteractionResponse.parseFrom(message.getContents().getContents().getValue());
        assertThat(response.getContents().getContentType(), is("application/json"));
        assertThat(response.getContents().getContent().getValue().toStringUtf8(), is("{}"));
        assertThat(response.getContents().getContentTypeHint(), is(Plugin.Body.ContentTypeHint.TEXT));

        assertThat(response.getGeneratorsCount(), is(2));
        Map<String, Plugin.Generator> generatorsMap = response.getGeneratorsMap();
        assertThat(generatorsMap.keySet(), is(equalTo(Set.of("$.test.one", "$.test.two"))));
        assertThat(generatorsMap.get("$.test.one").getType(), is(equalTo("DateTime")));
        assertThat(generatorsMap.get("$.test.one").getValues().getFieldsMap().get("format").getStringValue(), is(equalTo("YYYY-MM-DD")));
    }
}
