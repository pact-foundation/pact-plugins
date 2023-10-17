/* tslint:disable:no-unused-expression no-empty */
import { Verifier } from "@pact-foundation/pact";
import * as path from "path";
import { bindServer, getServer } from "../provider";

describe("Plugins", () => {
  const PORT = 50051;
  const HOST = "127.0.0.1";

  describe("Verify gRPC with Pact", () => {
    describe("Area Calculator Provider", () => {
      beforeEach(async () => {
        const grpcServer = getServer();
        bindServer(grpcServer, [HOST, PORT].join(":"));

        console.log("Started grpcServer:", HOST);
      });

      it("validates consumer requests", () => {
        const v = new Verifier({
          logLevel: "info",
          transports: [
            {
              port: PORT,
              protocol: "grpc",
            },
          ],
          provider: "area-calculator-provider",
          pactUrls: [
            path.join(
              __dirname,
              "../",
              "pacts",
              "area-calculator-consumer-js-area-calculator-provider.json"
            ),
          ],
        });

        return v.verifyProvider();
      });
    });
  });
});
