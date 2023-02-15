package io.pact.example.grpc.provider;

import io.grpc.Context;
import io.grpc.Contexts;
import io.grpc.ForwardingServerCall;
import io.grpc.ServerCall;
import io.grpc.ServerCallHandler;
import io.grpc.ServerInterceptor;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import metadatatest.Metadata;
import metadatatest.TestGrpc;

import java.util.regex.Pattern;

import static io.grpc.Metadata.ASCII_STRING_MARSHALLER;
import static io.pact.example.grpc.provider.TestImpl.TestImplInterceptor.USER_IDENTITY;

public class TestImpl extends TestGrpc.TestImplBase {
  @Override
  public void validateToken(Metadata.ValidateTokenRequest request, StreamObserver<Metadata.ValidateTokenResult> responseObserver) {
    String auth = USER_IDENTITY.get().toString();
    if (auth.startsWith("CAT")) {
      Metadata.ValidateTokenResult reply = Metadata.ValidateTokenResult
        .newBuilder()
        .setOk(true)
        .build();
      responseObserver.onNext(reply);
      responseObserver.onCompleted();
    } else {
      responseObserver.onError(new RuntimeException("Auth code is not valid"));
    }
  }

  public static class TestImplInterceptor implements ServerInterceptor {
    public static final io.grpc.Metadata.Key<String> authKey = io.grpc.Metadata.Key.of("Auth", ASCII_STRING_MARSHALLER);
    public static final io.grpc.Metadata.Key<String> codeKey = io.grpc.Metadata.Key.of("code", ASCII_STRING_MARSHALLER);
    public static final Pattern authCheckPatten = Pattern.compile("[A-Z]{3}\\d+");
    public static final Context.Key<Object> USER_IDENTITY = Context.key("identity");

    @Override
    public <ReqT, RespT> ServerCall.Listener<ReqT> interceptCall(
      ServerCall<ReqT, RespT> call,
      final io.grpc.Metadata headers,
      ServerCallHandler<ReqT, RespT> next
    ) {
      String auth = headers.get(authKey);
      if (auth == null || auth.isEmpty()) {
        call.close(Status.UNAUTHENTICATED, new io.grpc.Metadata());
        return new ServerCall.Listener() {};
      } else {
        if (authCheckPatten.matcher(auth).matches()) {
          Context context = Context.current().withValue(USER_IDENTITY, auth);
          ForwardingServerCall.SimpleForwardingServerCall<ReqT, RespT> forwardingServerCall = new ForwardingServerCall.SimpleForwardingServerCall<>(call) {
            @Override
            public void sendHeaders(io.grpc.Metadata headers) {
              headers.put(codeKey, "123456789");
              super.sendHeaders(headers);
            }
          };
          return Contexts.interceptCall(context, forwardingServerCall, headers, next);
        } else {
          call.close(Status.UNAUTHENTICATED, new io.grpc.Metadata());
          return new ServerCall.Listener() {};
        }
      }
    }
  }
}
