package io.pact.example.grpc.provider

import area_calculator.AreaCalculator
import area_calculator.CalculatorGrpcKt
import io.grpc.Metadata
import io.grpc.ServerBuilder
import io.grpc.ServerCall
import io.grpc.ServerCallHandler
import io.grpc.ServerInterceptor
import io.grpc.Status
import mu.KLogging
import java.util.regex.Pattern
import kotlin.math.PI
import kotlin.math.pow
import kotlin.math.sqrt

class Server {
    private val server = ServerBuilder.forPort(0)
        .intercept(CalculatorInterceptor())
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

class CalculatorInterceptor : ServerInterceptor {
    val authKey: Metadata.Key<String> = Metadata.Key.of("Auth", Metadata.ASCII_STRING_MARSHALLER)
    val authCheckPatten: Pattern = Pattern.compile("[A-Z]{3}\\d{3}")

    override fun <ReqT : Any?, RespT : Any?> interceptCall(
        call: ServerCall<ReqT, RespT>,
        headers: Metadata?,
        next: ServerCallHandler<ReqT, RespT>?
    ): ServerCall.Listener<ReqT> {
        val auth = headers?.get(authKey)
        return if (auth.isNullOrEmpty()) {
            call.close(Status.UNAUTHENTICATED, Metadata())
            NoOpListener()
        } else {
            if (authCheckPatten.matcher(auth).matches()) {
                if (next != null) {
                    next.startCall(call, headers)
                } else {
                    NoOpListener()
                }
            } else {
                call.close(Status.UNAUTHENTICATED, Metadata())
                NoOpListener()
            }
        }
    }
}

class NoOpListener<ReqT> : ServerCall.Listener<ReqT>()

class CalculatorService : CalculatorGrpcKt.CalculatorCoroutineImplBase() {
    override suspend fun calculateOne(request: AreaCalculator.ShapeMessage): AreaCalculator.AreaResponse {
        val area = when (request.shapeCase) {
            AreaCalculator.ShapeMessage.ShapeCase.SQUARE -> {
                logger.debug { "Got a SQUARE ${request.square}" }
                request.square.edgeLength.pow(2)
            }
            AreaCalculator.ShapeMessage.ShapeCase.RECTANGLE -> {
                logger.debug { "Got a RECTANGLE ${request.rectangle}" }
                if (request.rectangle.width == 3.0f || request.rectangle.length == 3.0f) {
                    throw RuntimeException("Provider state values were not injected")
                }
                request.rectangle.width * request.rectangle.length
            }
            AreaCalculator.ShapeMessage.ShapeCase.CIRCLE -> {
                logger.debug { "Got a CIRCLE ${request.circle}" }
                PI.toFloat() * request.circle.radius.pow(2)
            }
            AreaCalculator.ShapeMessage.ShapeCase.TRIANGLE -> {
                logger.debug { "Got a TRIANGLE ${request.triangle}" }
                val p = (request.triangle.edgeA + request.triangle.edgeB + request.triangle.edgeC) / 2.0f
                sqrt(p * (p - request.triangle.edgeA) * (p - request.triangle.edgeB) * (p - request.triangle.edgeC))
            }
            AreaCalculator.ShapeMessage.ShapeCase.PARALLELOGRAM -> {
                logger.debug { "Got a PARALLELOGRAM ${request.parallelogram}" }
                request.parallelogram.baseLength * request.parallelogram.height
            }
            else -> throw RuntimeException("Invalid request")
        }
        logger.debug { "Calculated area = $area" }
        return AreaCalculator.AreaResponse.newBuilder().addValue(area).build()
    }

    override suspend fun calculateMulti(request: AreaCalculator.AreaRequest): AreaCalculator.AreaResponse {
        var builder = AreaCalculator.AreaResponse.newBuilder()
        for (shape in request.shapesList) {
            val area = calculateOne(shape)
            builder = builder.addValue(area.getValue(0))
        }
        val response = builder.build()
        logger.debug { "Response = $response" }
        return response
    }

    companion object : KLogging()
}

fun main() {
    val server = Server()
    server.start()
    server.blockUntilShutdown()
}
