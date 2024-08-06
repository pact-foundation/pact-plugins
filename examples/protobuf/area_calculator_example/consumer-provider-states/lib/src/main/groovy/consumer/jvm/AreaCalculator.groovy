package consumer.jvm

import groovy.transform.CompileStatic
import org.slf4j.Logger
import org.slf4j.LoggerFactory

import static java.lang.Math.sqrt

@CompileStatic
class AreaCalculator {
    private static final Logger LOGGER = LoggerFactory.getLogger(AreaCalculator)

    static area_calculator.AreaCalculator.AreaResponse calculate(area_calculator.AreaCalculator.ShapeMessage request) {
        // Make sure the generators are working
        if (request.created == "2000-01-01") {
            throw new RuntimeException("Invalid created date '${request.created}'")
        }
        
        float area = 0
        switch (request.shapeCase) {
            case area_calculator.AreaCalculator.ShapeMessage.ShapeCase.SQUARE:
                LOGGER.debug("Got a SQUARE ${request.square}")
                area = (float) (request.square.edgeLength**2)
                break
            case area_calculator.AreaCalculator.ShapeMessage.ShapeCase.RECTANGLE:
                LOGGER.debug("Got a RECTANGLE ${request.rectangle}")
                area = (float) (request.rectangle.width * request.rectangle.length)
                break
            case area_calculator.AreaCalculator.ShapeMessage.ShapeCase.CIRCLE:
                LOGGER.debug("Got a CIRCLE ${request.circle}")
                area = (float) (Math.PI * request.circle.radius**2)
                break
            case area_calculator.AreaCalculator.ShapeMessage.ShapeCase.TRIANGLE:
                LOGGER.debug("Got a TRIANGLE ${request.triangle}")
                float p = (float) ((request.triangle.edgeA + request.triangle.edgeB + request.triangle.edgeC) / 2.0f)
                area = (float) sqrt(p * (p - request.triangle.edgeA) * (p - request.triangle.edgeB) * (p - request.triangle.edgeC))
                break
            case area_calculator.AreaCalculator.ShapeMessage.ShapeCase.PARALLELOGRAM:
                LOGGER.debug("Got a PARALLELOGRAM ${request.parallelogram}")
                area = (float) (request.parallelogram.baseLength * request.parallelogram.height)
                break
            default:
                throw new RuntimeException("Invalid request")
        }
        LOGGER.debug("Calculated area = $area")
        area_calculator.AreaCalculator.AreaResponse.newBuilder().setValue(area).build()
    }
}
