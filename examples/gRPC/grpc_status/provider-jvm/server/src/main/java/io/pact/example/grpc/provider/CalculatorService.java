package io.pact.example.grpc.provider;

import area_calculator.CalculatorGrpc;
import area_calculator.GrpcStatus;
import io.grpc.Status;
import io.grpc.StatusRuntimeException;
import io.grpc.stub.StreamObserver;

public class CalculatorService extends CalculatorGrpc.CalculatorImplBase {
  @Override
  public void calculate(GrpcStatus.ShapeMessage request, StreamObserver<GrpcStatus.AreaResponse> responseObserver) {
    GrpcStatus.AreaResponse.Builder builder = GrpcStatus.AreaResponse.newBuilder();
    try {
      if (request.hasCircle()) {
        builder.addValue((float) (Math.PI * Math.pow(request.getCircle().getRadius(), 2.0)));
      } else if (request.hasParallelogram()) {
        throw new StatusRuntimeException(Status.UNIMPLEMENTED.withDescription("we do not currently support parallelograms"));
      } else if (request.hasRectangle()) {
        GrpcStatus.Rectangle rectangle = request.getRectangle();
        builder.addValue(rectangle.getWidth() * rectangle.getLength());
      } else if (request.hasSquare()) {
        builder.addValue((float) Math.pow(request.getSquare().getEdgeLength(), 2.0));
      } else if (request.hasTriangle()) {
        GrpcStatus.Triangle triangle = request.getTriangle();
        float p = (triangle.getEdgeA() + triangle.getEdgeB() + triangle.getEdgeC()) / 2.0f;
        builder.addValue((float) Math.sqrt(p * (p - triangle.getEdgeA()) * (p - triangle.getEdgeB()) * (p - triangle.getEdgeC())));
      } else {
        throw new StatusRuntimeException(Status.INVALID_ARGUMENT.withDescription("Invalid request: Not a valid shape"));
      }
      responseObserver.onNext(builder.build());
      responseObserver.onCompleted();
    } catch (StatusRuntimeException e) {
      responseObserver.onError(e);
    }
  }
}
