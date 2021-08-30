package io.pact.protobuf.plugin

import com.github.os72.protocjar.Protoc
import com.google.protobuf.DescriptorProtos
import mu.KLogging
import java.nio.file.Files
import java.nio.file.Path


object ProtoParser : KLogging() {
  fun parseProtoFile(protoFile: Path): DescriptorProtos.FileDescriptorSet {
    logger.debug { "Parsing protobuf proto file $protoFile" }

    val tmpDir = Path.of("./tmp/")
    tmpDir.toFile().mkdirs()
    val outFile = Files.createTempFile(tmpDir, null, null)

    val args = arrayOf("--include_std_types", "-o${outFile}", "-I${protoFile.parent}", "--include_imports", protoFile.toString())
    logger.debug { "Invoking protoc with ${args.toList()}" }
    if (Protoc.runProtoc(args) != 0) {
      throw RuntimeException("Failed to execute protoc")
    }

    return Files.newInputStream(outFile).use { DescriptorProtos.FileDescriptorSet.parseFrom(it) }
  }
}
