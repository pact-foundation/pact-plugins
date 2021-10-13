package io.pact.protobuf.plugin

import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import com.github.os72.protocjar.Protoc
import com.google.protobuf.DescriptorProtos
import io.pact.plugins.jvm.core.SystemExec
import io.pact.plugins.jvm.core.Utils
import mu.KLogging
import java.io.File
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import kotlin.io.path.deleteIfExists

object ProtoParser : KLogging() {
  fun parseProtoFile(protoFile: Path): DescriptorProtos.FileDescriptorSet {
    logger.debug { "Parsing protobuf proto file $protoFile" }

    val tmpDir = Path.of("./tmp/")
    tmpDir.toFile().mkdirs()
    val outFile = Files.createTempFile(tmpDir, null, null)

    val args = arrayOf("--include_std_types", "-o${outFile}", "-I${protoFile.parent}", "--include_imports", protoFile.toString())
    logger.debug { "Invoking bundled protoc with ${args.toList()}" }
    if (!runProtoc(args)) {
      logger.error { "Failed to run the bundled protoc, will try the system one" }
      when (val protocPath = Utils.lookForProgramInPath("protoc")) {
        is Ok -> {
          unpackWellKnownTypes(tmpDir)
          val systemArgs = args.drop(1).plus("-I./tmp").toTypedArray()
          logger.debug { "Executing: ${protocPath.value} ${systemArgs.toList()}" }
          when (val result = SystemExec.execute(protocPath.value.toString(), *systemArgs)) {
            is Ok -> {}
            is Err -> throw RuntimeException("Failed to execute protoc in the system path - ${result.error.first} ${result.error.second}")
          }
        }
        is Err -> throw RuntimeException("Failed to execute bundled protoc and did not find protoc in the system path")
      }
    }

    val fileDescriptorSet = Files.newInputStream(outFile).use { DescriptorProtos.FileDescriptorSet.parseFrom(it) }
    outFile.deleteIfExists()
    return fileDescriptorSet
  }

  private fun unpackWellKnownTypes(tmpDir: Path) {
    val includeDir = File(tmpDir.toFile(), "google/protobuf")
    includeDir.mkdirs()
    writeFile(includeDir, "struct.proto")
    writeFile(includeDir, "any.proto")
    writeFile(includeDir, "api.proto")
    writeFile(includeDir, "descriptor.proto")
    writeFile(includeDir, "duration.proto")
    writeFile(includeDir, "empty.proto")
    writeFile(includeDir, "field_mask.proto")
    writeFile(includeDir, "source_context.proto")
    writeFile(includeDir, "struct.proto")
    writeFile(includeDir, "timestamp.proto")
    writeFile(includeDir, "type.proto")
    writeFile(includeDir, "wrappers.proto")
  }

  private fun writeFile(includeDir: File, name: String) {
    File(includeDir, name).printWriter().use {
      it.write(javaClass.getResource("/google/protobuf/$name").readText())
    }
  }

  private fun runProtoc(args: Array<String>): Boolean {
    return try {
      Protoc.runProtoc(args) == 0
    } catch (e: IOException) {
      logger.error { "Protoc call failed with an exception - $e" }
      false
    }
  }
}
