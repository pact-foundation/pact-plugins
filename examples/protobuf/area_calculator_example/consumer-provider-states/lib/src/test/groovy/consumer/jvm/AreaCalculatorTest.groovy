package consumer.jvm

import au.com.dius.pact.consumer.groovy.messaging.PactSynchronousMessageBuilder
import au.com.dius.pact.consumer.groovy.messaging.SynchronousMessageBuilder
import au.com.dius.pact.core.matchers.generators.DefaultResponseGenerator
import au.com.dius.pact.core.model.generators.GeneratorTestMode
import spock.lang.Specification

import static au.com.dius.pact.consumer.dsl.BuilderUtils.filePath
import static consumer.jvm.AreaCalculator.calculate

class AreaCalculatorTest extends Specification {
    def "test area calculator client with message"() {
        given:
        def areaCalculatorService = new PactSynchronousMessageBuilder()
        areaCalculatorService {
            serviceConsumer 'area_calculator-consumer-jvm'
            hasPactWith 'area_calculator-provider'
            usingPlugin('protobuf')
            given("a rectangle")
            expectsToReceive('request for calculate shape area') { SynchronousMessageBuilder builder ->
                builder.testname('test area calculator client with message')
                builder.withPluginConfig([
                  "pact:proto": filePath('../../proto/area_calculator.proto'),
                  "pact:content-type": "application/protobuf",
                  "pact:proto-service": "Calculator/calculate",
                  "request": [
                    "rectangle": [
                      "length": "matching(number, fromProviderState('\${rectangleLength}', 3))",
                      "width": "matching(number, fromProviderState('\${rectangleWidth}', 4))"
                    ],
                    "created": "matching(date, 'yyyy-MM-dd', '2000-01-01')"
                  ],
                  "response": [
                    "value" : "matching(number, 12)"
                  ]
                ])
            }
        }

        when:
        def testResult = areaCalculatorService.run { message, pact ->
            def requestContents = DefaultResponseGenerator.INSTANCE.generateContents(message.request,
                [:], GeneratorTestMode.Consumer, pact.pluginData(), message.pluginConfiguration, true)

            def request = area_calculator.AreaCalculator.ShapeMessage.parseFrom(requestContents.contents.value)
            def response = area_calculator.AreaCalculator.AreaResponse.parseFrom(message.response.first().contents.value)
            def result = calculate(request)

            result.value == response.value
        }

        then:
        noExceptionThrown()
        testResult == [true]
    }
}
