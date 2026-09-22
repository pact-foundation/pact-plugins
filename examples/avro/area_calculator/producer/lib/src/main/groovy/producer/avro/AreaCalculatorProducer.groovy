package producer.avro

import org.apache.avro.Schema
import org.apache.avro.generic.GenericData
import org.apache.avro.generic.GenericDatumWriter
import org.apache.avro.generic.GenericRecord
import org.apache.avro.io.EncoderFactory

class AreaCalculatorProducer {
    private final Schema schema

    AreaCalculatorProducer(String schemaPath) {
        this.schema = new Schema.Parser().parse(new File(schemaPath))
    }

    byte[] calculateRectangleArea(float length, float width) {
        return encodeResponse("rectangle", length * width)
    }

    byte[] calculateCircleArea(float radius) {
        return encodeResponse("circle", (float) (Math.PI * radius * radius))
    }

    byte[] calculateSquareArea(float side) {
        return encodeResponse("square", side * side)
    }

    private byte[] encodeResponse(String shape, double value) {
        def record = new GenericData.Record(schema)
        record.put("shape", shape)
        record.put("value", (float) value)

        def writer = new GenericDatumWriter<GenericRecord>(schema)
        def baos = new ByteArrayOutputStream()
        def encoder = EncoderFactory.get().binaryEncoder(baos, null)
        writer.write(record, encoder)
        encoder.flush()
        return baos.toByteArray()
    }
}
