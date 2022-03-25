package io.pact.example.grpc.provider

import area_calculator.CalculatorGrpcKt
import io.grpc.ServerBuilder

class Server {
    private val server = ServerBuilder.forPort(0).addService(CalculatorService()).build()

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
}

class CalculatorService : CalculatorGrpcKt.CalculatorCoroutineImplBase() {

}

fun main() {
    val server = Server()
    server.start()
    server.blockUntilShutdown()
}
