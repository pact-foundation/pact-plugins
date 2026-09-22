'use strict';

const avro = require('avsc');
const fs = require('fs');

class AreaCalculatorClient {
  constructor(schemaPath) {
    const schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
    this.type = avro.Type.forSchema(schema);
  }

  processAreaResponse(bytes) {
    const buf = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes);
    return this.type.fromBuffer(buf);
  }
}

module.exports = { AreaCalculatorClient };
