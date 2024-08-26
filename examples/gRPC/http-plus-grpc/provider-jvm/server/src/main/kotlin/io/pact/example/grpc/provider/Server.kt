package io.pact.example.grpc.provider

import area_calculator.AreaCalculator
import area_calculator.CalculatorGrpcKt
import io.grpc.ServerBuilder
import kotlin.math.PI
import kotlin.math.pow
import kotlin.math.sqrt

class Server {
    private val server = ServerBuilder.forPort(0)
        .addService(CalculatorService()).build()

    fun start() {
        server.start()
        println("Started calculator service on ${server.port}")
        Runtime.getRuntime().addShutdownHook(
            Thread {
                println("*** shutting down gRPC server since JVM is shutting down")
                server.shutdownNow()
                println("*** server shut down")
            }
        )
    }

    fun stop() {
        server.shutdown()
    }

    fun blockUntilShutdown() {
        server.awaitTermination()
    }

    fun serverPort() = server.port
}

class CalculatorService : CalculatorGrpcKt.CalculatorCoroutineImplBase() {
    override suspend fun calculateOne(request: AreaCalculator.ShapeMessage): AreaCalculator.AreaResponse {
        val area = when (request.shapeCase) {
            AreaCalculator.ShapeMessage.ShapeCase.SQUARE -> {
                request.square.edgeLength.pow(2)
            }
            AreaCalculator.ShapeMessage.ShapeCase.RECTANGLE -> {
                request.rectangle.width * request.rectangle.length
            }
            AreaCalculator.ShapeMessage.ShapeCase.CIRCLE -> {
                PI.toFloat() * request.circle.radius.pow(2)
            }
            AreaCalculator.ShapeMessage.ShapeCase.TRIANGLE -> {
                val p = (request.triangle.edgeA + request.triangle.edgeB + request.triangle.edgeC) / 2.0f
                sqrt(p * (p - request.triangle.edgeA) * (p - request.triangle.edgeB) * (p - request.triangle.edgeC))
            }
            AreaCalculator.ShapeMessage.ShapeCase.PARALLELOGRAM -> {
                request.parallelogram.baseLength * request.parallelogram.height
            }
            else -> throw RuntimeException("Invalid request")
        }
        return AreaCalculator.AreaResponse.newBuilder().addValue(area).build()
    }

    override suspend fun calculateMulti(request: AreaCalculator.AreaRequest): AreaCalculator.AreaResponse {
        var builder = AreaCalculator.AreaResponse.newBuilder()
        for (shape in request.shapesList) {
            val area = calculateOne(shape)
            builder = builder.addValue(area.getValue(0))
        }
        val response = builder.build()
        return response
    }
}

fun main() {
    val server = Server()
    server.start()
    server.blockUntilShutdown()
}
