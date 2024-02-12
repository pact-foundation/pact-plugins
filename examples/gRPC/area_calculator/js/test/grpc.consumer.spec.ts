/* tslint:disable:no-unused-expression no-empty */
import chai from "chai";
import chaiAsPromised from "chai-as-promised";
import { SpecificationVersion, PactV4, LogLevel } from "@pact-foundation/pact";
import { client, getShapeMessage } from "../consumer";
import * as path from "path";
chai.use(chaiAsPromised);

const { expect } = chai;

describe("Plugins - grpc Protocol", () => {
  describe("TCP interface", () => {
    const pact = new PactV4({
      consumer: "area-calculator-consumer-js",
      provider: "area-calculator-provider",
      spec: SpecificationVersion.SPECIFICATION_VERSION_V4,
      logLevel: (process.env.LOG_LEVEL as LogLevel) || "error",
    });
    const HOST = "127.0.0.1";
    describe("with grpc protocol", async () => {
      it("generates a pact with success", () => {
        const grpcMessage = {
          "pact:proto": path.resolve("./proto/area_calculator.proto"),
          "pact:proto-service": "Calculator/calculateOne",
          "pact:content-type": "application/protobuf",
          request: {
            rectangle: {
              length: "matching(number, 3)",
              width: "matching(number, 4)",
            },
          },
          response: { value: ["matching(number, 12)"] },
        };

        return pact
          .addSynchronousInteraction("A gRPC calculateOne request")
          .usingPlugin({
            plugin: "protobuf",
            version: "0.3.13",
          })
          .withPluginContents(JSON.stringify(grpcMessage), "application/grpc")
          .startTransport("grpc", HOST)
          .executeTest(async (tc) => {
            const delay = (ms: number) =>
              new Promise((resolve) => setTimeout(resolve, ms));
            const grpcClient = client([HOST, tc.port].join(":"));
            getShapeMessage(grpcClient, {
              rectangle: {
                length: 8,
                width: 4,
              },
            });

            return await delay(1000);
          });
      });
    });
  });
});
