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

import java.util.Map;

import static au.com.dius.pact.consumer.dsl.PactBuilder.filePath;
import static org.hamcrest.MatcherAssert.assertThat;
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
              "proto", filePath("../../../proto/plugin.proto"),
              "message-type", "InitPluginRequest",
              "content-type", "application/protobuf",
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
              "proto", filePath("../../../proto/plugin.proto"),
              "message-type", "ConfigureInteractionResponse",
              "content-type", "application/protobuf",
              "contents", Map.of(
                "contentType", "notEmpty('application/json')",
                "content", "matching(contentType, 'application/json', '{}')",
                "contentTypeHint", "matching(equalTo, 'TEXT')"
              )
//                // All matching rules to apply
//                map<string, MatchingRules> rules = 2;
//                // Generators to apply
//                map<string, Generator> generators = 3;
//                // For message interactions, any metadata to be applied
//                google.protobuf.Struct messageMetadata = 4;
//                // Plugin specific data to be persisted in the pact file
//                PluginConfiguration pluginConfiguration = 5;
//                // Markdown/HTML formatted text representation of the interaction
//                string interactionMarkup = 6;
//                // Type of markup used
//                enum MarkupType {
//                    // CommonMark format
//                    COMMON_MARK = 0;
//                    // HTML format
//                    HTML = 1;
//                }
//                MarkupType interactionMarkupType = 7;
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
    }
}
