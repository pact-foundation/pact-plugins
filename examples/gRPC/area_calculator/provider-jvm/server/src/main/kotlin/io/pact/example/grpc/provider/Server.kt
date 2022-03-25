package io.pact.example.grpc.provider

class Server {
    val greeting: String
        get() {
            return "Hello World!"
        }
}

fun main() {
    println(Server().greeting)
}
