'use strict';

const path = require('path');
const { makeConsumerAsyncMessagePact } = require('@pact-foundation/pact-core');
const { AreaCalculatorClient } = require('./area_calculator_client');

const schemaPath = path.resolve(__dirname, '..', '..', 'schema', 'area_response.avsc');
const pactDir   = path.resolve(__dirname, '..', '..', 'pacts');

// FfiSpecificationVersion.SPECIFICATION_VERSION_V4 = 5
const SPEC_V4 = 5;

const client = new AreaCalculatorClient(schemaPath);

async function runPactTest(description, pluginConfig, verify) {
  const pact = makeConsumerAsyncMessagePact(
    'area-calculator-node-consumer',
    'area-calculator-producer',
    SPEC_V4,
    'info'
  );
  pact.addPlugin('avro', '0.1.0-dev');

  const message = pact.newAsynchronousMessage(description);
  message.withPluginRequestInteractionContents(
    'avro/binary',
    JSON.stringify(pluginConfig)
  );

  const raw     = JSON.parse(message.reifyMessage());
  // For plugin-generated binary bodies, contents is { content, contentType, encoded }
  const bytes   = Buffer.from(raw.contents.content, 'base64');

  await verify(bytes);

  pact.writePactFile(pactDir, true);
  pact.cleanupPlugins();
}

describe('Area Calculator Consumer', () => {
  it('receives an area response for a rectangle', async () => {
    await runPactTest(
      'an area response for a rectangle',
      {
        'pact:avro'        : schemaPath,
        'pact:record-name' : 'AreaResponse',
        'pact:content-type': 'avro/binary',
        'shape'            : "notEmpty('rectangle')",
        'value'            : "matching(decimal, 12.0)",
      },
      (bytes) => {
        const decoded = client.processAreaResponse(bytes);
        expect(decoded.shape).toBe('rectangle');
        expect(decoded.value).toBeGreaterThan(0);
      }
    );
  });

  it('receives an area response for a circle', async () => {
    await runPactTest(
      'an area response for a circle',
      {
        'pact:avro'        : schemaPath,
        'pact:record-name' : 'AreaResponse',
        'pact:content-type': 'avro/binary',
        'shape'            : "notEmpty('circle')",
        'value'            : "matching(decimal, 78.5)",
      },
      (bytes) => {
        const decoded = client.processAreaResponse(bytes);
        expect(decoded.shape).toBe('circle');
        expect(decoded.value).toBeGreaterThan(0);
      }
    );
  });
});
