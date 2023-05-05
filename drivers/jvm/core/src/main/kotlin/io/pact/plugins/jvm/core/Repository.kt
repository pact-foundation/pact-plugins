package io.pact.plugins.jvm.core

import au.com.dius.pact.core.support.Result
import au.com.dius.pact.core.support.Result.Err
import au.com.dius.pact.core.support.Result.Ok
import com.moandjiezana.toml.Toml
import mu.KLogging
import org.apache.commons.codec.digest.DigestUtils
import org.apache.hc.client5.http.fluent.Request
import java.io.File

/**
 * Class representing the plugin repository index file
 */
data class PluginRepositoryIndex(
  /**
   * Version of this index file
   */
  val indexVersion: Int,

  /**
   * File format version of the index file
   */
  val formatVersion: Int,

  /**
   * Timestamp (in UTC) that the file was created/updated
   */
  val timestamp: String,

  /**
   * Plugin entries
   */
  val entries: Map<String, PluginEntry>
) {
  fun lookupPluginVersion(name: String, version: String?): PluginVersion? {
    val entry = entries[name]
    return if (entry != null) {
      val versionToInstall = if (version != null) {
        logger.debug { "Installing plugin $name/$version from index" }
        version
      } else {
        logger.debug { "Installing plugin $name/latest from index" }
        entry.latestVersion
      }
      entry.versions.find { it.version == versionToInstall }
    } else {
      null
    }
  }

  companion object: KLogging() {
    fun from(toml: Toml): PluginRepositoryIndex {
      val table = toml.getTable("entries")
      return PluginRepositoryIndex(
        toml.getLong("index_version").toInt(),
        toml.getLong("format_version").toInt(),
        toml.getString("timestamp"),
        table.toMap().mapValues {
          PluginEntry.from(table.getTable(it.key))
        }
      )
    }
  }
}

/**
 * Class to store the plugin version entries
 */
data class PluginEntry(
  /**
   * Name of the plugin
   */
  val name: String,

  /**
   * Latest version
   */
  val latestVersion: String,

  /**
   * All the plugin versions
   */
  val versions : List<PluginVersion>
) {
  companion object {
    fun from(toml: Toml): PluginEntry {
      return PluginEntry(
        toml.getString("name"),
        toml.getString("latest_version"),
        toml.getTables("versions").map {
          PluginVersion.from(it)
        }
      )
    }
  }
}

/**
 * Struct to store the plugin versions
 */
data class PluginVersion(
  /**
   * Version of the plugin
   */
  val version: String,

  /**
   * Source the manifest was loaded from
   */
  val source: ManifestSource,

  /**
   * Plugin Manifest
   */
  val manifest : PactPluginManifest? = null
) {
  companion object {
    fun from(toml: Toml): PluginVersion {
      return PluginVersion(
        toml.getString("version"),
        ManifestSource.from(toml.getTable("source"))
      )
    }
  }
}

/**
 * Source that the plugin is loaded from
 */
sealed class ManifestSource(open val value: String) {
  /**
   * Unknown source
   */
  data class Unknown(override val value: String): ManifestSource(value)

  /**
   * Loaded from a file
   */
  data class File(val file: String): ManifestSource(file)

  /**
   * Loaded from a GitHub release
   */
  data class GitHubRelease(val url: String): ManifestSource(url)

  companion object {
    fun from(toml: Toml): ManifestSource {
      return when(toml.getString("type")) {
        "GitHubRelease" -> GitHubRelease(toml.getString("value"))
        "File" -> File(toml.getString("value"))
        else -> Unknown(toml.getString("value"))
      }
    }
  }
}

interface Repository {
  /**
   * Fetches the repository index, first trying GitHub, then a local cached copy, falling back to the
   * built-in index
   */
  fun fetchRepositoryIndex(): Result<PluginRepositoryIndex, String>

  /**
   * Loads the local cached copy of the index
   */
  fun loadLocalIndex(): Result<PluginRepositoryIndex, String>

  /**
   * Fetches the committed index from GitHub
   */
  fun fetchIndexFromGithub(): Result<PluginRepositoryIndex, String>
}

open class DefaultRepository: Repository, KLogging() {
  override fun fetchRepositoryIndex(): Result<PluginRepositoryIndex, String> {
    return fetchIndexFromGithub()
      .orElse {
        logger.warn { "Was not able to load index from GitHub - $it" }
        loadLocalIndex()
      }
      .orElse {
        logger.warn { "Was not able to load local index, will use built in one - $it" }
        defaultIndex()
      }
  }

  override fun loadLocalIndex(): Result<PluginRepositoryIndex, String> {
    val pluginDir = File(DefaultPluginManager.pluginInstallDirectory())
    if (!pluginDir.exists()) {
      return Err("Plugin directory does not exist")
    }

    val repositoryFile = File(pluginDir, "repository.index")
    val calculatedSha = calculateSha(repositoryFile)
    val expectedSha = loadSha(repositoryFile)
    if (calculatedSha != expectedSha) {
      return Err("SHA256 digest does not match: expected $expectedSha but got $calculatedSha")
    }

    logger.debug { "Loading local index file" }
    val toml = Toml().read(repositoryFile.readText())
    return Ok(PluginRepositoryIndex.from(toml))
  }

  private fun loadSha(file: File): String {
    val shaFile = getShaFileForRepositoryFile(file)
    return shaFile.readText()
  }

  private fun getShaFileForRepositoryFile(file: File): File {
    val filenameBase = file.name
    val shaFileName = "${filenameBase}.sha256"
    return File(file.parent, shaFileName)
  }

  private fun calculateSha(file: File): String {
    return DigestUtils.getSha256Digest().digest(file.readBytes()).toHex()
  }

  override fun fetchIndexFromGithub(): Result<PluginRepositoryIndex, String> {
    logger.info { "Fetching index from github" }
    try {
      val indexContent =
        getUrl("https://raw.githubusercontent.com/pact-foundation/pact-plugins/main/repository/repository.index")
          .execute()
          .returnContent()
          .asString()
      val indexSha =
        getUrl("https://raw.githubusercontent.com/pact-foundation/pact-plugins/main/repository/repository.index.sha256")
          .execute()
          .returnContent()
          .asString()
      val calculated = DigestUtils.getSha256Digest().digest(indexContent.toByteArray()).toHex()
      if (calculated != indexSha) {
        return Err("SHA256 digest from GitHub does not match: expected $calculated but got $indexSha")
      }

      cacheIndex(indexContent, indexSha)

      val toml = Toml().read(indexContent)
      return Ok(PluginRepositoryIndex.from(toml))
    } catch (ex: RuntimeException) {
      logger.error(ex) { "Failed to fetch index from GitHub" }
      return Err("Failed to fetch index from GitHub: ${ex.message}")
    }
  }

  fun cacheIndex(indexContent: String, indexSha: String) {
    val pluginDir = File(DefaultPluginManager.pluginInstallDirectory())
    if (!pluginDir.exists()) {
      pluginDir.mkdirs()
    }
    val repositoryFile = File(pluginDir, "repository.index")
    repositoryFile.writeText(indexContent)
    val shaFile = File(pluginDir, "repository.index.sha256")
    shaFile.writeText(indexSha)
  }

  open fun getUrl(uri: String): Request {
    return Request.get(uri)
      .userAgent("pact-plugin-driver")
  }

  open fun defaultIndex(): Result<PluginRepositoryIndex, String> {
    val stream = DefaultRepository::class.java.getResourceAsStream("/repository.index")
    return if (stream != null) {
      try {
        val toml = Toml().read(stream)
        Ok(PluginRepositoryIndex.from(toml))
      } catch (ex: RuntimeException) {
        logger.error(ex) { "Could not load default index" }
        Err("Could not load default index, ${ex.message}")
      }
    } else {
      Err("Could not load default index, resource was not found")
    }
  }
}

// TODO: replace with version from Pact-JVM
private fun <V, E> Result<V, E>.orElse(function: (E) -> Result<V, E>): Result<V, E> {
  return when (this) {
    is Err -> function(error)
    is Ok -> this
  }
}
