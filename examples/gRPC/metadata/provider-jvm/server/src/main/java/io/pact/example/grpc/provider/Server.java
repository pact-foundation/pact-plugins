package io.pact.example.grpc.provider;

import io.grpc.Grpc;
import io.grpc.InsecureServerCredentials;
import io.grpc.ServerInterceptors;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

public class Server {
    private static final Logger LOGGER = LoggerFactory.getLogger(Server.class);
    private io.grpc.Server server;

    public void start() throws IOException {
        server = Grpc.newServerBuilderForPort(50051, InsecureServerCredentials.create())
          .addService(ServerInterceptors.intercept(new TestImpl(), new TestImpl.TestImplInterceptor()))
          .build()
          .start();
        LOGGER.info("Started gRPC service on 50051");
        Runtime.getRuntime().addShutdownHook(
          new Thread(() -> {
              System.err.println("*** shutting down gRPC server since JVM is shutting down");
              try {
                  stop();
              } catch (InterruptedException e) {
                  throw new RuntimeException(e);
              }
              System.err.println("*** server shut down");
          })
        );
    }

    public void stop() throws InterruptedException {
        server.shutdown().awaitTermination();
    }

    public void blockUntilShutdown() throws InterruptedException {
        server.awaitTermination();
    }

    public int serverPort() {
        return server.getPort();
    }

    public static void main(String[] args) throws IOException, InterruptedException {
        final Server server = new Server();
        server.start();
        server.blockUntilShutdown();
    }
}
