# Example JVM provider project

This example project has a server implementation in Kotlin for the area calculator service call:

```protobuf
  rpc calculate (ShapeMessage) returns (AreaResponse) {}
```

The Gradle Protobuf plugin is used to generate the gRPC classes for the calculate service call and the [Kotlin Server
class](server/src/main/kotlin/io/pact/example/grpc/provider/Server.kt) implements the calculate method.

## gRPC plugin

To run the test in this project, it requires the gRPC plugin to be installed. See the [documentation on that plugin](https://github.com/pactflow/pact-protobuf-plugin#installation).

## Pact verification test

There is a [Pact verification test](server/src/test/java/io/pact/example/grpc/provider/PactVerificationTest.java) written in Java and JUint 5 that can verify the Kotlin server using a Pact file from
one of the consumer projects.

## Verifying the gRPC server using Verifier CLI

The server can also be verified by using the [Pact Verifier CLI](https://github.com/pact-foundation/pact-reference/tree/master/rust/pact_verifier_cli).

For this to work, the server first needs to be started. Then the `pact_verifier_cli` can be used with one of the Pact files
from the consumer projects to verify the server.

### First, start the server

We can use the Gradle run task for that:

```console
gRPC/area_calculator/provider-jvm: 
❯ ./gradlew run
Starting a Gradle Daemon (subsequent builds will be faster)

> Task :server:run
14:21:59.427 [main] DEBUG io.netty.util.internal.logging.InternalLoggerFactory - Using SLF4J as the default logging framework
14:21:59.439 [main] DEBUG io.netty.util.internal.PlatformDependent0 - -Dio.netty.noUnsafe: false
14:21:59.439 [main] DEBUG io.netty.util.internal.PlatformDependent0 - Java version: 11
14:21:59.441 [main] DEBUG io.netty.util.internal.PlatformDependent0 - sun.misc.Unsafe.theUnsafe: available
14:21:59.442 [main] DEBUG io.netty.util.internal.PlatformDependent0 - sun.misc.Unsafe.copyMemory: available
14:21:59.442 [main] DEBUG io.netty.util.internal.PlatformDependent0 - java.nio.Buffer.address: available
14:21:59.444 [main] DEBUG io.netty.util.internal.PlatformDependent0 - direct buffer constructor: unavailable
java.lang.UnsupportedOperationException: Reflective setAccessible(true) disabled
        at io.netty.util.internal.ReflectionUtil.trySetAccessible(ReflectionUtil.java:31)
        at io.netty.util.internal.PlatformDependent0$4.run(PlatformDependent0.java:239)
        at java.base/java.security.AccessController.doPrivileged(Native Method)
        at io.netty.util.internal.PlatformDependent0.<clinit>(PlatformDependent0.java:233)
        at io.netty.util.internal.PlatformDependent.isAndroid(PlatformDependent.java:294)
        at io.netty.util.internal.PlatformDependent.<clinit>(PlatformDependent.java:93)
        at io.netty.util.AsciiString.<init>(AsciiString.java:223)
        at io.netty.util.AsciiString.<init>(AsciiString.java:210)
        at io.netty.util.AsciiString.cached(AsciiString.java:1401)
        at io.netty.util.AsciiString.<clinit>(AsciiString.java:48)
        at io.grpc.netty.Utils.<clinit>(Utils.java:78)
        at io.grpc.netty.NettyServerBuilder.<clinit>(NettyServerBuilder.java:83)
        at io.grpc.netty.NettyServerProvider.builderForPort(NettyServerProvider.java:40)
        at io.grpc.netty.NettyServerProvider.builderForPort(NettyServerProvider.java:25)
        at io.grpc.ServerBuilder.forPort(ServerBuilder.java:44)
        at io.pact.example.grpc.provider.Server.<init>(Server.kt:12)
        at io.pact.example.grpc.provider.ServerKt.main(Server.kt:71)
        at io.pact.example.grpc.provider.ServerKt.main(Server.kt)
14:21:59.445 [main] DEBUG io.netty.util.internal.PlatformDependent0 - java.nio.Bits.unaligned: available, true
14:21:59.446 [main] DEBUG io.netty.util.internal.PlatformDependent0 - jdk.internal.misc.Unsafe.allocateUninitializedArray(int): unavailable
java.lang.IllegalAccessException: class io.netty.util.internal.PlatformDependent0$6 cannot access class jdk.internal.misc.Unsafe (in module java.base) because module java.base does not export jdk.internal.misc to unnamed module @f0f2775
        at java.base/jdk.internal.reflect.Reflection.newIllegalAccessException(Reflection.java:361)
        at java.base/java.lang.reflect.AccessibleObject.checkAccess(AccessibleObject.java:591)
        at java.base/java.lang.reflect.Method.invoke(Method.java:558)
        at io.netty.util.internal.PlatformDependent0$6.run(PlatformDependent0.java:353)
        at java.base/java.security.AccessController.doPrivileged(Native Method)
        at io.netty.util.internal.PlatformDependent0.<clinit>(PlatformDependent0.java:344)
        at io.netty.util.internal.PlatformDependent.isAndroid(PlatformDependent.java:294)
        at io.netty.util.internal.PlatformDependent.<clinit>(PlatformDependent.java:93)
        at io.netty.util.AsciiString.<init>(AsciiString.java:223)
        at io.netty.util.AsciiString.<init>(AsciiString.java:210)
        at io.netty.util.AsciiString.cached(AsciiString.java:1401)
        at io.netty.util.AsciiString.<clinit>(AsciiString.java:48)
        at io.grpc.netty.Utils.<clinit>(Utils.java:78)
        at io.grpc.netty.NettyServerBuilder.<clinit>(NettyServerBuilder.java:83)
        at io.grpc.netty.NettyServerProvider.builderForPort(NettyServerProvider.java:40)
        at io.grpc.netty.NettyServerProvider.builderForPort(NettyServerProvider.java:25)
        at io.grpc.ServerBuilder.forPort(ServerBuilder.java:44)
        at io.pact.example.grpc.provider.Server.<init>(Server.kt:12)
        at io.pact.example.grpc.provider.ServerKt.main(Server.kt:71)
        at io.pact.example.grpc.provider.ServerKt.main(Server.kt)
14:21:59.447 [main] DEBUG io.netty.util.internal.PlatformDependent0 - java.nio.DirectByteBuffer.<init>(long, int): unavailable
14:21:59.447 [main] DEBUG io.netty.util.internal.PlatformDependent - sun.misc.Unsafe: available
14:21:59.464 [main] DEBUG io.netty.util.internal.PlatformDependent - maxDirectMemory: 8350859264 bytes (maybe)
14:21:59.464 [main] DEBUG io.netty.util.internal.PlatformDependent - -Dio.netty.tmpdir: /tmp (java.io.tmpdir)
14:21:59.464 [main] DEBUG io.netty.util.internal.PlatformDependent - -Dio.netty.bitMode: 64 (sun.arch.data.model)
14:21:59.466 [main] DEBUG io.netty.util.internal.PlatformDependent - -Dio.netty.maxDirectMemory: -1 bytes
14:21:59.466 [main] DEBUG io.netty.util.internal.PlatformDependent - -Dio.netty.uninitializedArrayAllocationThreshold: -1
14:21:59.466 [main] DEBUG io.netty.util.internal.CleanerJava9 - java.nio.ByteBuffer.cleaner(): available
14:21:59.467 [main] DEBUG io.netty.util.internal.PlatformDependent - -Dio.netty.noPreferDirect: false
14:21:59.558 [main] DEBUG io.netty.channel.MultithreadEventLoopGroup - -Dio.netty.eventLoopThreads: 32
14:21:59.573 [main] DEBUG io.netty.util.internal.InternalThreadLocalMap - -Dio.netty.threadLocalMap.stringBuilder.initialSize: 1024
14:21:59.573 [main] DEBUG io.netty.util.internal.InternalThreadLocalMap - -Dio.netty.threadLocalMap.stringBuilder.maxSize: 4096
14:21:59.578 [main] DEBUG io.netty.channel.nio.NioEventLoop - -Dio.netty.noKeySetOptimization: false
14:21:59.578 [main] DEBUG io.netty.channel.nio.NioEventLoop - -Dio.netty.selectorAutoRebuildThreshold: 512
14:21:59.592 [main] DEBUG io.netty.util.internal.PlatformDependent - org.jctools-core.MpscChunkedArrayQueue: available
14:21:59.618 [main] DEBUG io.netty.util.ResourceLeakDetector - -Dio.netty.leakDetection.level: simple
14:21:59.618 [main] DEBUG io.netty.util.ResourceLeakDetector - -Dio.netty.leakDetection.targetRecords: 4
14:21:59.619 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.numHeapArenas: 32
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.numDirectArenas: 32
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.pageSize: 8192
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.maxOrder: 11
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.chunkSize: 16777216
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.smallCacheSize: 256
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.normalCacheSize: 64
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.maxCachedBufferCapacity: 32768
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.cacheTrimInterval: 8192
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.cacheTrimIntervalMillis: 0
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.useCacheForAllThreads: true
14:21:59.620 [main] DEBUG io.netty.buffer.PooledByteBufAllocator - -Dio.netty.allocator.maxCachedByteBuffersPerChunk: 1023
14:21:59.660 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.channel.DefaultChannelId - -Dio.netty.processId: 139445 (auto-detected)
14:21:59.662 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.util.NetUtil - -Djava.net.preferIPv4Stack: false
14:21:59.662 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.util.NetUtil - -Djava.net.preferIPv6Addresses: false
14:21:59.663 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.util.NetUtilInitializations - Loopback interface: lo (lo, 0:0:0:0:0:0:0:1%lo)
14:21:59.664 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.util.NetUtil - /proc/sys/net/core/somaxconn: 4096
14:21:59.665 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.channel.DefaultChannelId - -Dio.netty.machineId: 9c:b6:d0:ff:fe:3f:8e:1a (auto-detected)
14:21:59.683 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.buffer.ByteBufUtil - -Dio.netty.allocator.type: pooled
14:21:59.683 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.buffer.ByteBufUtil - -Dio.netty.threadLocalDirectBufferSize: 0
14:21:59.683 [grpc-nio-boss-ELG-1-1] DEBUG io.netty.buffer.ByteBufUtil - -Dio.netty.maxThreadLocalCharBufferSize: 16384
Started calculator service on 37757
<===========--> 87% EXECUTING [30s]
> :server:run
```

Note the port that is printed out above (`37757`), we need to pass that on to the verifier CLI.

### Second, run the pact_verifier_cli

We use the Pact file from the consumer project and the port from the step above.

```console
gRPC/area_calculator/provider-jvm: 
❯ pact_verifier_cli -f ../consumer-jvm/build/pacts/protobuf-consumer-area-calculator-provider.json -p 37757 -l none
2022-05-02T04:29:36.636972Z  INFO main pact_verifier: Pact file requires plugins, will load those now
2022-05-02T04:29:36.638556Z  WARN tokio-runtime-worker pact_plugin_driver::metrics: 

Please note:
We are tracking this plugin load anonymously to gather important usage statistics.
To disable tracking, set the 'pact_do_not_track' environment variable to 'true'.


2022-05-02T04:29:36.873435Z  INFO                 main pact_verifier: Running provider verification for 'calculate rectangle area request'

Verifying a pact between protobuf-consumer and area-calculator-provider

  calculate rectangle area request

  Test Name: io.pact.example.grpc.consumer.PactConsumerTest.calculateRectangleArea(MockServer, SynchronousMessages)

  Given a Calculator/calculate request
      with an input .area_calculator.ShapeMessage message
      will return an output .area_calculator.AreaResponse message [OK]

```
