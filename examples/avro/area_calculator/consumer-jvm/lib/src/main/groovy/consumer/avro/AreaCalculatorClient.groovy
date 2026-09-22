package consumer.avro

import org.apache.avro.Schema
import org.apache.avro.generic.GenericDatumReader
import org.apache.avro.generic.GenericRecord
import org.apache.avro.io.DecoderFactory

class AreaCalculatorClient {
    private final Schema schema

    AreaCalculatorClient(String schemaPath) {
        this.schema = new Schema.Parser().parse(new File(schemaPath))
    }

    AreaResult processAreaResponse(byte[] bytes) {
        def reader = new GenericDatumReader<GenericRecord>(schema)
        def decoder = DecoderFactory.get().binaryDecoder(bytes, null)
        def record = reader.read(null, decoder)
        return new AreaResult(
            shape: record.get("shape").toString(),
            value: record.get("value") as float
        )
    }
}
