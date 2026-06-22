package io.pact.plugins.jvm.core

/**
 * Utilities for configuring plugin-related log output.
 *
 * The gRPC transport stack (Netty, grpc-netty) produces high-volume trace output that is
 * unrelated to plugin behaviour. Add the following to your logback.xml (or equivalent) to
 * suppress it:
 *
 * ```xml
 * <logger name="io.grpc.netty" level="WARN"/>
 * <logger name="io.netty"      level="WARN"/>
 * <logger name="io.grpc"       level="WARN"/>
 * ```
 *
 * Or in log4j2.xml:
 *
 * ```xml
 * <Logger name="io.grpc.netty" level="WARN" additivity="false"/>
 * <Logger name="io.netty"      level="WARN" additivity="false"/>
 * <Logger name="io.grpc"       level="WARN" additivity="false"/>
 * ```
 *
 * [TRANSPORT_NOISE_LOGGERS] lists the logger name prefixes to cap.
 */
object PluginLogging {
  /**
   * Logger name prefixes that generate high-volume trace output unrelated to plugin
   * behaviour. These should be capped at WARN in any project that embeds the plugin driver.
   */
  val TRANSPORT_NOISE_LOGGERS: List<String> = listOf(
    "io.grpc.netty",
    "io.netty",
    "io.grpc"
  )
}
