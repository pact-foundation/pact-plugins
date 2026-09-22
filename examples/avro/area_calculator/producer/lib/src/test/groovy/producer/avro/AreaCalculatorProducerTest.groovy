package producer.avro

import au.com.dius.pact.core.model.ContentTypeHint
import au.com.dius.pact.provider.MessageAndMetadata
import au.com.dius.pact.provider.PactVerifyProvider
import au.com.dius.pact.provider.junit5.MessageTestTarget
import au.com.dius.pact.provider.junit5.PactVerificationContext
import au.com.dius.pact.provider.junit5.PactVerificationInvocationContextProvider
import au.com.dius.pact.provider.junitsupport.Provider
import au.com.dius.pact.provider.junitsupport.loader.PactFolder
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.TestTemplate
import org.junit.jupiter.api.extension.ExtendWith

import java.nio.file.Paths

@Provider("area-calculator-producer")
@PactFolder("../../pacts")
class AreaCalculatorProducerTest {

    private final String schemaPath = Paths.get("../../schema/area_response.avsc").toAbsolutePath().toString()
    private final AreaCalculatorProducer producer = new AreaCalculatorProducer(schemaPath)

    @TestTemplate
    @ExtendWith(PactVerificationInvocationContextProvider.class)
    void testTemplate(PactVerificationContext context) {
        context.verifyInteraction()
    }

    @SuppressWarnings("JUnitMalformedDeclaration")
    @BeforeEach
    void setupTest(PactVerificationContext context) {
        context.setTarget(new MessageTestTarget())
    }

    @PactVerifyProvider("an area response for a rectangle")
    MessageAndMetadata rectangleAreaResponse() {
        return new MessageAndMetadata(
            producer.calculateRectangleArea(3.0f, 4.0f),
            Map.of(
                "contentType", "avro/binary; record=AreaResponse",
                "contentTypeHint", ContentTypeHint.BINARY
            )
        )
    }

    @PactVerifyProvider("an area response for a circle")
    MessageAndMetadata circleAreaResponse() {
        return new MessageAndMetadata(
            producer.calculateCircleArea(5.0f),
            Map.of(
                "contentType", "avro/binary; record=AreaResponse",
                "contentTypeHint", ContentTypeHint.BINARY
            )
        )
    }
}
