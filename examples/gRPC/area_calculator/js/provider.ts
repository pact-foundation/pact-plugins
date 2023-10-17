#!/usr/bin/env ts-node
import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { ProtoGrpcType } from "./proto/area_calculator";
import * as path from "path";
import { CalculatorHandlers } from "./proto/area_calculator/Calculator";
import { AreaRequest__Output } from "./proto/area_calculator/AreaRequest";
import { AreaResponse } from "./proto/area_calculator/AreaResponse";
import { ShapeMessage__Output } from "./proto/area_calculator/ShapeMessage";

const HOST = "127.0.0.1:9090";

const exampleServer: CalculatorHandlers = {
  calculateOne(
    call: grpc.ServerUnaryCall<ShapeMessage__Output, AreaResponse>,
    callback: grpc.sendUnaryData<AreaResponse>
  ) {
    if (call.request) {
      console.log(
        `(server) Got client message: ${JSON.stringify(call.request)}`
      );
    }
    const message = call.request;
    const messageType = Object.keys(message)[0];
    const messageContents = Object.values(message)[0];
    switch (messageType) {
      case "rectangle":
        console.log(messageContents);

        return callback(null, {
          // @ts-ignore
          value: [messageContents.length * messageContents.width],
        });
      case "square":
        return callback(null, {
          // @ts-ignore
          value: [messageContents.edgeLength * messageContents.edgeLength],
        });
      case "circle":
        return {
          // @ts-ignore
          value: [
            Math.PI *
              messageContents.circle.radius *
              messageContents.circle.radius,
          ],
        };
      case "parallelogram":
        return callback(null, {
          value: [
            // @ts-ignore
            messageContents.parallelogram.base_length *
              messageContents.parallelogram.height,
          ],
        });
      case "triangle":
        const p = // @ts-ignore
          (messageContents.triangle.edgeA +
            // @ts-ignore
            messageContents.triangle.edgeB + // @ts-ignore
            messageContents.triangle?.edgeC) /
          2.0;
        const area = Math.sqrt(
          p * // @ts-ignore
            (p - messageContents.triangle.edgeA) * // @ts-ignore
            (p - messageContents.triangle.edgeB) * // @ts-ignore
            (p - messageContents.triangle.edgeC)
        );
        return callback(null, { value: [area] });
      default:
        throw new Error(`Error: not a valid shape ${message}`);
    }
  },
  calculateMulti(
    call: grpc.ServerUnaryCall<AreaRequest__Output, AreaResponse>,
    callback: grpc.sendUnaryData<AreaResponse>
  ) {},
};

export function getServer(): grpc.Server {
  const packageDefinition = protoLoader.loadSync(
    path.resolve("./proto/area_calculator.proto")
  );
  const proto = grpc.loadPackageDefinition(
    packageDefinition
  ) as unknown as ProtoGrpcType;
  const server = new grpc.Server();
  server.addService(proto.area_calculator.Calculator.service, exampleServer);
  return server;
}

export const bindServer = (server: grpc.Server, host: string) => {
  server.bindAsync(
    host,
    grpc.ServerCredentials.createInsecure(),
    (err: Error | null, port: number) => {
      if (err) {
        console.error(`Server error: ${err.message}`);
      } else {
        console.log(`Server bound on port: ${port}`);
        server.start();
      }
    }
  );
};

if (require.main === module) {
  const server = getServer();
  bindServer(server, HOST);
}
