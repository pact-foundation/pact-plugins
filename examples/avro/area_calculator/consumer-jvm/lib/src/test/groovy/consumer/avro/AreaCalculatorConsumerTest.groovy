package consumer.avro

import au.com.dius.pact.consumer.dsl.PactBuilder
import au.com.dius.pact.consumer.junit5.PactConsumerTestExt
import au.com.dius.pact.consumer.junit5.PactTestFor
import au.com.dius.pact.consumer.junit5.ProviderType
import au.com.dius.pact.core.model.PactSpecVersion
import au.com.dius.pact.core.model.V4Interaction
import au.com.dius.pact.core.model.V4Pact
import au.com.dius.pact.core.model.annotations.Pact
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.extension.ExtendWith

import java.nio.file.Paths

@ExtendWith(PactConsumerTestExt.class)
@PactTestFor(providerName = "area-calculator-producer", providerType = ProviderType.ASYNCH, pactVersion = PactSpecVersion.V4)
class AreaCalculatorConsumerTest {

    private final String schemaPath = Paths.get("../../schema/area_response.avsc").toAbsolutePath().toString()
    private final AreaCalculatorClient client = new AreaCalculatorClient(schemaPath)

    @Pact(consumer = "area-calculator-jvm-consumer")
    V4Pact rectangleAreaPact(PactBuilder builder) {
        return builder
            .usingPlugin("avro")
            .expectsToReceive("an area response for a rectangle", "core/interaction/message")
            .with([
                "message.contents": [
                    "pact:avro"        : schemaPath,
                    "pact:record-name" : "AreaResponse",
                    "pact:content-type": "avro/binary",
                    "shape"            : "notEmpty('rectangle')",
                    "value"            : "matching(decimal, 12.0)"
                ]
            ])
            .toPact()
    }

    @Pact(consumer = "area-calculator-jvm-consumer")
    V4Pact circleAreaPact(PactBuilder builder) {
        return builder
            .usingPlugin("avro")
            .expectsToReceive("an area response for a circle", "core/interaction/message")
            .with([
                "message.contents": [
                    "pact:avro"        : schemaPath,
                    "pact:record-name" : "AreaResponse",
                    "pact:content-type": "avro/binary",
                    "shape"            : "notEmpty('circle')",
                    "value"            : "matching(decimal, 78.5)"
                ]
            ])
            .toPact()
    }

    @Test
    @PactTestFor(pactMethod = "rectangleAreaPact")
    void verifyRectangleAreaResponse(V4Interaction.AsynchronousMessage message) {
        def bytes = message.getContents().getContents().getValue()
        def result = client.processAreaResponse(bytes)
        assert result.shape == "rectangle"
        assert result.value > 0
    }

    @Test
    @PactTestFor(pactMethod = "circleAreaPact")
    void verifyCircleAreaResponse(V4Interaction.AsynchronousMessage message) {
        def bytes = message.getContents().getContents().getValue()
        def result = client.processAreaResponse(bytes)
        assert result.shape == "circle"
        assert result.value > 0
    }
}
