package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.Result
import au.com.dius.pact.core.support.isNotEmpty
import io.github.oshai.kotlinlogging.KLogging
import org.apache.commons.codec.digest.DigestUtils
import org.apache.commons.io.FilenameUtils
import org.apache.commons.io.IOUtils
import org.apache.commons.lang3.SystemUtils.IS_OS_LINUX
import org.apache.commons.lang3.SystemUtils.IS_OS_MAC_OSX
import org.apache.commons.lang3.SystemUtils.IS_OS_UNIX
import org.apache.commons.lang3.SystemUtils.IS_OS_WINDOWS
import org.apache.commons.lang3.SystemUtils.OS_ARCH
import org.apache.commons.lang3.SystemUtils.OS_NAME
import org.apache.hc.client5.http.fluent.Request
import org.rauschig.jarchivelib.ArchiveFormat
import org.rauschig.jarchivelib.ArchiverFactory
import java.io.File
import java.io.StringReader
import java.util.zip.GZIPInputStream
import javax.json.Json
import javax.json.JsonObject
import javax.json.JsonString
import javax.json.JsonValue
import java.io.BufferedReader
import java.io.InputStreamReader

interface PluginDownloader {
  /**
   * Downloads the plugin from the GitHub URL and installs it
   */
  fun installPluginFromUrl(sourceUrl: String): Result<PactPluginManifest, String>
}

object DefaultPluginDownloader: PluginDownloader, KLogging() {
  override fun installPluginFromUrl(sourceUrl: String): Result<PactPluginManifest, String> {
    val response = when (val response = fetchJsonFromUrl(sourceUrl)) {
      is Result.Ok -> response.value
      is Result.Err -> return response
    }
    return if (response is JsonObject) {
      val tagName = response["tag_name"]
      if (tagName != null) {
        val tag = asString(tagName)
        logger.debug { "Found tag $tag" }
        val url = if (sourceUrl.endsWith("/latest")) {
          sourceUrl.removeSuffix("/latest")
        } else {
          sourceUrl.removeSuffix("/tag/$tag")
        }
        when (val manifestJsonResult = downloadJsonFromGithub(url, tag, "pact-plugin.json")) {
          is Result.Ok -> {
            val manifestJson = manifestJsonResult.value
            if (manifestJson is JsonObject) {
              val pluginDirs = File(DefaultPluginManager.pluginInstallDirectory())
              if (!pluginDirs.exists()) {
                pluginDirs.mkdirs()
              }
              val pluginName = asString(manifestJson["name"])
              val pluginVersion = asString(manifestJson["version"])
              val pluginDir = File(pluginDirs, "${pluginName}-${pluginVersion}")
              pluginDir.mkdir()
              val manifest = DefaultPactPluginManifest.fromJson(pluginDir, manifestJson)
              logger.debug { "Loaded manifest from GitHub - $manifest" }
              val manifestFile = File(pluginDir, "pact-plugin.json")
              manifestFile.writeText(manifestJson.toString())

              logger.debug { "Installing plugin ${manifest.name} version ${manifest.version}" }
              when (val result = downloadPluginExecutable(manifest, pluginDir, url, tag)) {
                is Result.Ok -> Result.Ok(manifest)
                is Result.Err -> result
              }
            } else {
              Result.Err("Downloaded manifest is not a valid JSON document")
            }
          }
          is Result.Err -> manifestJsonResult
        }
      } else {
        Result.Err("GitHub release page does not have a valid tag_name attribute")
      }
    } else {
      Result.Err("Response from source is not a valid JSON from a GitHub release page")
    }
  }

  private fun downloadPluginExecutable(
    manifest: PactPluginManifest,
    pluginDir: File,
    url: String,
    tag: String
  ): Result<File, String> {
    val (os, arch) = detectOsAndArch();

    val muslExt = if (isMusl()) "-musl" else ""

    // Check for a single exec .gz file
    val ext = if (os == "windows") ".exe" else ""
    val gzFile = "pact-${manifest.name}-plugin-$os-$arch$ext.gz"
    val shaFileName = "pact-${manifest.name}-plugin-$os-$arch$ext.gz.sha256"

    if (isMusl()){
      val gzFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt$ext.gz"
      val shaFileNameMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt$ext.gz.sha256"

      if (githubFileExists(url, tag, gzFileMusl)){
        val gzFile = gzFileMusl
        val shaFileName = shaFileNameMusl
      } else {
        logger.warn { "musl detected, but no musl specific plugin implementation found - you may experience issues" }
      }
    }
    if (githubFileExists(url, tag, gzFile)) {
      logger.debug { "Found a GZipped file $gzFile" }
      when (val fileResult = downloadFileFromGithub(url, tag, gzFile, pluginDir)) {
        is Result.Ok -> {
          if (githubFileExists(url, tag, shaFileName)) {
            val shaFile = downloadFileFromGithub(url, tag, shaFileName, pluginDir)
            if (shaFile is Result.Ok) {
              when (val shaResult = checkSha(fileResult.value, shaFile.value)) {
                is Result.Ok -> shaFile.value.delete()
                is Result.Err -> {
                  shaFile.value.delete()
                  fileResult.value.delete()
                  return shaResult
                }
              }
            }
          }

          return when (val gunzipResult = gunzipFile(fileResult.value, pluginDir, manifest, ext)) {
            is Result.Ok -> {
              if (IS_OS_UNIX) {
                gunzipResult.value.setExecutable(true)
              }
              Result.Ok(fileResult.value)
            }
            is Result.Err -> gunzipResult
          }
        }
        is Result.Err -> return fileResult
      }
    }

    // Check for an arch specific Zip file
    val archZipFile = "pact-${manifest.name}-plugin-$os-$arch.zip"
    val archZipShaFile = "pact-${manifest.name}-plugin-$os-$arch.zip.sha256"

    if (isMusl()){
      val archZipFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.zip"
      val archZipShaFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.zip.sha256"

      if (githubFileExists(url, tag, archZipFileMusl)){
        val archZipFile = archZipFileMusl
        val archZipShaFile = archZipShaFileMusl
      } else {
        logger.warn { "musl detected, but no musl specific plugin implementation found - you may experience issues" }
      }
    }

    if (githubFileExists(url, tag, archZipFile)) {
      return downloadZipFile(pluginDir, url, tag, archZipFile, archZipShaFile)
    }

    // Check for a Zip file
    val zipFile = "pact-${manifest.name}-plugin.zip"
    val zipShaFile = "pact-${manifest.name}-plugin.zip.sha256"
    if (githubFileExists(url, tag, zipFile)) {
      return downloadZipFile(pluginDir, url, tag, zipFile, zipShaFile)
    }

    // Check for a tar.gz file
    val tarGzFile = "pact-${manifest.name}-plugin.tar.gz"
    val tarGzShaFile = "pact-${manifest.name}-plugin.tar.gz.sha256"
    if (githubFileExists(url, tag, tarGzFile)) {
      return downloadTarGzfile(pluginDir, url, tag, tarGzFile, tarGzShaFile)
    }

    // Check for an arch specific tar.gz file
    val archTarGzFile = "pact-${manifest.name}-plugin-$os-$arch.tar.gz"
    val archTarGzShaFile = "pact-${manifest.name}-plugin-$os-$arch.tag.gz.sha256"

    if (isMusl()){
      val archTarGzFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.tar.gz"
      val archTarGzShaFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.tag.gz.sha256"

      if (githubFileExists(url, tag, archTarGzFileMusl)){
        val archTarGzFile = archTarGzFileMusl
        val archTarGzShaFile = archTarGzShaFileMusl
      } else {
        logger.warn { "musl detected, but no musl specific plugin implementation found - you may experience issues" }
      }
    }

    if (githubFileExists(url, tag, archTarGzFile)) {
      return downloadTarGzfile(pluginDir, url, tag, archTarGzFile, archTarGzShaFile)
    }
    // Check for a tgz file
    val tgzFile = "pact-${manifest.name}-plugin.tgz"
    val tgzShaFile = "pact-${manifest.name}-plugin.tgz.sha256"
    if (githubFileExists(url, tag, tgzFile)) {
      return downloadTarGzfile(pluginDir, url, tag, tgzFile, tgzShaFile)
    }

    // Check for an arch specific tgz file
    val archTgzFile = "pact-${manifest.name}-plugin-$os-$arch.tgz"
    val archTgzShaFile = "pact-${manifest.name}-plugin-$os-$arch.tgz.sha256"

    if (isMusl()){
      val archTgzFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.tgz"
      val archTgzShaFileMusl = "pact-${manifest.name}-plugin-$os-$arch$muslExt.tgz.sha256"
  
      if (githubFileExists(url, tag, archTgzFileMusl)){
        val archTgzFile = archTgzFileMusl
        val archTgzShaFileMusl = archTgzShaFileMusl
      } else {
        logger.warn { "musl detected, but no musl specific plugin implementation found - you may experience issues" }
      }
    }

    if (githubFileExists(url, tag, archTgzFile)) {
      return downloadTarGzfile(pluginDir, url, tag, archTgzFile, archTgzShaFile)
    }

    return Result.Err("Did not find a matching file pattern on GitHub to install")
  }

  private fun downloadZipFile(
    pluginDir: File,
    url: String,
    tag: String,
    zipFile: String,
    zipShaFile: String
  ): Result<File, String> {
    logger.debug { "Found a Zip file $zipFile" }
    when (val fileResult = downloadFileFromGithub(url, tag, zipFile, pluginDir)) {
      is Result.Ok -> {
        if (githubFileExists(url, tag, zipShaFile)) {
          val shaFile = downloadFileFromGithub(url, tag, zipShaFile, pluginDir)
          if (shaFile is Result.Ok) {
            when (val shaResult = checkSha(fileResult.value, shaFile.value)) {
              is Result.Ok -> shaFile.value.delete()
              is Result.Err -> {
                shaFile.value.delete()
                fileResult.value.delete()
                return shaResult
              }
            }
          }
        }

        val archiver = ArchiverFactory.createArchiver(ArchiveFormat.ZIP)
        archiver.extract(fileResult.value, pluginDir)
        return Result.Ok(fileResult.value)
      }
      is Result.Err -> return fileResult
    }
  }

  private fun gunzipFile(
    gzFile: File,
    pluginDir: File,
    manifest: PactPluginManifest,
    ext: String
  ): Result<File, String> {
     val file = if (ext.isNotEmpty()) {
       File(pluginDir, manifest.entryPoint)
     } else {
       File(pluginDir, FilenameUtils.removeExtension(manifest.entryPoint) + ext)
     }
    return try {
      GZIPInputStream(gzFile.inputStream()).use {
        file.outputStream().use { out ->
          val bytes = IOUtils.copy(it, out)
          logger.debug { "Wrote $bytes bytes to $file" }
        }
      }
      gzFile.delete()
      Result.Ok(file)
    } catch (ex: RuntimeException) {
      logger.error(ex) { "Failed to unzip download file" }
      Result.Err("Failed to unzip download file: ${ex.message}")
    }
  }
 
  private fun downloadTarGzfile(    
  pluginDir: File,
  url: String,
  tag: String,
  tarGzFile: String,
  tarGzShaFile: String): Result<File, String> {
    when (val fileResult = downloadFileFromGithub(url, tag, tarGzFile, pluginDir)) {
      is Result.Ok -> {
        if (githubFileExists(url, tag, tarGzShaFile)) {
          val shaFile = downloadFileFromGithub(url, tag, tarGzShaFile, pluginDir)
          if (shaFile is Result.Ok) {
            when (val shaResult = checkSha(fileResult.value, shaFile.value)) {
              is Result.Ok -> shaFile.value.delete()
              is Result.Err -> {
                shaFile.value.delete()
                fileResult.value.delete()
                return shaResult
              }
            }
          }
        }

        val archiver = ArchiverFactory.createArchiver("tar", "gz")
        archiver.extract(fileResult.value, pluginDir)
        return Result.Ok(fileResult.value)
      }
      is Result.Err -> return fileResult
    }
  }

  private fun checkSha(file: File, shaFile: File): Result<Unit, String> {
    return try {
      logger.debug { "Checking SHA of downloaded file: ${file.name} against ${shaFile.name}" }
      val sha = shaFile.readText().split(" ").first()
      logger.debug { "Downloaded SHA $sha" }

      val calculated = DigestUtils.getSha256Digest().digest(file.readBytes()).toHex()
      logger.debug { "Calculated SHA $calculated" }
      if (calculated == sha) {
        Result.Ok(Unit)
      } else {
        Result.Err("Downloaded file $file has a checksum mismatch: $sha != $calculated")
      }
    } catch (ex: RuntimeException) {
      logger.error(ex) { "Failed to check SHA for download file" }
      Result.Err("Failed to check SHA for download file: ${ex.message}")
    }
  }

  private fun downloadFileFromGithub(
    baseUrl: String,
    tag: String,
    filename: String,
    pluginDir: File
  ): Result<File, String> {
    val url = "$baseUrl/download/$tag/$filename"
    logger.debug { "Downloading file from $url" }

    val file = File(pluginDir, filename)
    return try {
      Request.get(url)
        .userAgent("pact-plugin-driver")
        .execute()
        .saveContent(file)
      logger.debug { "File downloaded OK (${file.length()} bytes)" }
      Result.Ok(file)
    } catch (ex: RuntimeException) {
      logger.error(ex) { "Failed to download file from GitHub" }
      Result.Err("Failed to download file from GitHub: ${ex.message}")
    }
  }

  private fun githubFileExists(url: String, tag: String, filename: String): Boolean {
    return try {
      val status = Request.head("$url/download/$tag/$filename")
        .userAgent("pact-plugin-driver")
        .execute()
        .returnResponse()
        .code
      status in 200..299
    } catch (ex: RuntimeException) {
      logger.debug(ex) { "Request to GitHub failed" }
      false
    }
  }

  private fun isMusl(): Boolean {
    try {
      val process = Runtime.getRuntime().exec("ldd /bin/ls | grep 'musl'").inputStream.bufferedReader().readText()
      return process.contains("musl")
    } catch (ex: Exception) {
      return false
    }
  }

  private fun detectOsAndArch(): Pair<String, String> {
    val osArch = normalizeArch(OS_ARCH)
    logger.debug { "Detected Arch: $osArch" }
    val os = if (IS_OS_LINUX) {
      "linux"
    } else if (IS_OS_WINDOWS) {
      "windows"
    } else if (IS_OS_MAC_OSX) {
      "osx"
    } else {
      OS_NAME
    }
    logger.debug { "Detected OS: $os" }
    return os to osArch
  }

  private fun asString(value: JsonValue?): String {
    return when (value?.valueType) {
      JsonValue.ValueType.STRING -> (value as JsonString).string
      null -> "null"
      else -> value.toString()
    }
  }

  private fun downloadJsonFromGithub(url: String, tag: String, fileName: String)
    = fetchJsonFromUrl("$url/download/$tag/$fileName")

  private fun fetchJsonFromUrl(sourceUrl: String): Result<JsonValue, String> {
    logger.info { "Fetching root document for source '$sourceUrl'" }
    return try {
      val json = Request.get(sourceUrl)
        .userAgent("pact-plugin-driver")
        .addHeader("accept", "application/json")
        .execute()
        .returnContent()
        .asString()
      logger.debug { "Got response $json" }
      Result.Ok(Json.createReader(StringReader(json)).readValue())
    } catch (ex: RuntimeException) {
      logger.error(ex) { "Failed to fetch JSON from URL" }
      Result.Err("Failed to fetch JSON from URL: ${ex.message}")
    }
  }

  // Taken from https://github.com/trustin/os-maven-plugin/blob/master/src/main/java/kr/motd/maven/os/Detector.java#L192
  // Copyright 2014 Trustin Heuiseung Lee.
  //
  // Licensed under the Apache License, Version 2.0 (the "License");
  // you may not use this file except in compliance with the License.
  // You may obtain a copy of the License at
  //
  //      http://www.apache.org/licenses/LICENSE-2.0
  //
  //  Unless required by applicable law or agreed to in writing, software
  //  distributed under the License is distributed on an "AS IS" BASIS,
  //  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  //  See the License for the specific language governing permissions and
  //  limitations under the License.
  //

  private val xf8664Regex = "^(x8664|amd64|ia32e|em64t|x64)$".toRegex()
  private val xf8632Regex = "^(x8632|x86|i[3-6]86|ia32|x32)$".toRegex()
  private val itanium64Regex = "^(ia64w?|itanium64)$".toRegex()
  private val sparc32Regex = "^(sparc|sparc32)$".toRegex()
  private val sparc64Regex = "^(sparcv9|sparc64)$".toRegex()
  private val arm32Regex = "^(arm|arm32)$".toRegex()
  private val mips32Regex = "^(mips|mips32)$".toRegex()
  private val mipselRegex = "^(mipsel|mips32el)$".toRegex()
  private val ppc32Regex = "^(ppc|ppc32)$".toRegex()
  private val ppcle32Regex = "^(ppcle|ppc32le)$".toRegex()
  private val riscvRegex = "^(riscv|riscv32)$".toRegex()

  private fun normalizeArch(value: String?): String {
    val arch = normalize(value)
    return when {
      xf8664Regex.matches(arch) -> "x86_64"
      xf8632Regex.matches(arch) -> "x86_32"
      itanium64Regex.matches(arch) -> "itanium_64"
      "ia64n" == value -> "itanium_32"
      sparc32Regex.matches(arch) -> "sparc_32"
      sparc64Regex.matches(arch) -> "sparc_64"
      arm32Regex.matches(arch) -> "arm_32"
      "aarch64" == value -> "aarch64"
      mips32Regex.matches(arch) -> "mips_32"
      mipselRegex.matches(arch) -> "mipsel_32"
      "mips64" == value -> "mips_64"
      "mips64el" == value -> "mipsel_64"
      ppc32Regex.matches(arch) -> "ppc_32"
      ppcle32Regex.matches(arch) -> "ppcle_32"
      "ppc64" == value -> "ppc_64"
      "ppc64le" == value -> "ppcle_64"
      "s390" == value -> "s390_32"
      "s390x" == value -> "s390_64"
      riscvRegex.matches(arch) -> "riscv"
      "riscv64" == value -> "riscv64"
      "e2k" == value -> "e2k"
      "loongarch64" == value -> "loongarch64"
      else -> "unknown"
    }
  }

  private fun normalize(value: String?)
    = value?.lowercase()?.replace("[^a-z0-9]+".toRegex(), "") ?: ""
}
