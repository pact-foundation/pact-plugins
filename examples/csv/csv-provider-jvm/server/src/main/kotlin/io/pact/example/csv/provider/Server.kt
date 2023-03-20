package io.pact.example.csv.provider

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class Server

@Suppress("SpreadOperator")
fun main(args: Array<String>) {
    runApplication<Server>(*args)
}
