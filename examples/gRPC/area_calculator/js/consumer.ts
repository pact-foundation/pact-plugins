#!/usr/bin/env ts-node
import * as grpc from "@grpc/grpc-js";
import * as protoLoader from "@grpc/proto-loader";
import { ProtoGrpcType } from "./proto/area_calculator";
import { CalculatorClient } from "./proto/area_calculator/Calculator";
import { AreaResponse } from "./proto/area_calculator/AreaResponse";
import { ShapeMessage } from "./proto/area_calculator/ShapeMessage";

const packageDefinition = protoLoader.loadSync("./proto/area_calculator.proto");
const proto = grpc.loadPackageDefinition(
  packageDefinition
) as unknown as ProtoGrpcType;

export const client = (host: string) => {
  console.log(host);
  return new proto.area_calculator.Calculator(
    host,
    grpc.credentials.createInsecure()
  );
};
const deadline = new Date();
deadline.setSeconds(deadline.getSeconds() + 5);
const grpClient = client("127.0.0.1:9090");
grpClient.waitForReady(deadline, (error?: Error) => {
  if (error) {
    console.log(`Client connect error: ${error.message}`);
  } else {
    getShapeMessage(grpClient, {
      rectangle: {
        length: 8,
        width: 4,
      },
    });
  }
});
export function getShapeMessage(client: CalculatorClient, shape: ShapeMessage) {
  return client.calculateOne(
    shape,
    (error?: grpc.ServiceError | null, serverMessage?: AreaResponse) => {
      if (error) {
        console.error(error.message);
        return error.message;
      } else if (serverMessage) {
        console.log(`(client) Got server message: ${serverMessage.value}`);
        return serverMessage.value;
      }
    }
  );
}
