'use strict';

const path = require('path');
const { PactV4 } = require('@pact-foundation/pact');
const { AreaCalculatorClient } = require('./area_calculator_client');

const schemaPath = path.resolve(__dirname, '..', '..', 'schema', 'area_response.avsc');
const pactDir   = path.resolve(__dirname, '..', '..', 'pacts');

const client = new AreaCalculatorClient(schemaPath);

const consumer = new PactV4({
  consumer: 'area-calculator-node-consumer',
  provider: 'area-calculator-producer',
  dir: pactDir,
  logLevel: 'info',
});

const pluginConfig = (shape, value) => JSON.stringify({
  'pact:avro'        : schemaPath,
  'pact:record-name' : 'AreaResponse',
  'pact:content-type': 'avro/binary',
  'shape'            : `notEmpty('${shape}')`,
  'value'            : `matching(decimal, ${value})`,
});

describe('Area Calculator Consumer', () => {
  it('receives an area response for a rectangle', () => {
    return consumer
      .addAsynchronousInteraction()
      .usingPlugin({ plugin: 'avro', version: '0.1.0-dev' })
      .expectsToReceive('an area response for a rectangle')
      .withPluginContents(pluginConfig('rectangle', 12.0), 'avro/binary')
      .executeTest(async (m) => {
        const bytes = Buffer.from(m.contents.content, 'base64');
        const decoded = client.processAreaResponse(bytes);
        expect(decoded.shape).toBe('rectangle');
        expect(decoded.value).toBeGreaterThan(0);
      });
  });

  it('receives an area response for a circle', () => {
    return consumer
      .addAsynchronousInteraction()
      .usingPlugin({ plugin: 'avro', version: '0.1.0-dev' })
      .expectsToReceive('an area response for a circle')
      .withPluginContents(pluginConfig('circle', 78.5), 'avro/binary')
      .executeTest(async (m) => {
        const bytes = Buffer.from(m.contents.content, 'base64');
        const decoded = client.processAreaResponse(bytes);
        expect(decoded.shape).toBe('circle');
        expect(decoded.value).toBeGreaterThan(0);
      });
  });
});
