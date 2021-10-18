# Guide to writing a Pact plugin

Pact plugins are essentially gRPC servers that run as child processes to the main Pact process (whether in a consumer
test or during provider verification). They are designed to be stateless and respond to requests from the Pact framework
that is running the tests. They can be written in any language that has gRPC support, but ideally should
be written in a language that has minimal system dependencies.

**IMPORTANT NOTE:** Please keep the end users in mind when selecting a language to write a plugin in. If you use, say
Java, that means any user who uses your plugin needs to have a JDK installed on their machines and CI servers, as well
as anything that verifies a Pact file created using that plugin even if the provider is written in a different language.

There are two prototype example plugins, one for [CSV](../plugins/csv) and one for [Protobuf](../plugins/protobuf). 
You can find examples of consumer and provider tests using these plugins in the [examples](../examples) in this repository.
The CSV plugin is written in Rust and the Protobuf one in Kotlin. 

## Plugin Interface

The first version of the plugin interface (version 1) supports adding matchers and generators for different types
of content. Later versions will expand to add other things like protocol and transport implementations, but for now
the plugins can only provide support for new types of content. This is for things like request/response bodies and
message payloads and are based on specified MIME types. Refer to [Content Matchers and Generators](content-matcher-design.md) 
for more details on the interface and gRPC methods that need to be implemented.

You can find the [proto file](../proto/plugin.proto) that defines the plugin interface in the proto directory. Your 
plugin will need to implement this interface.

When the plugin starts up, it needs to write a small JSON message to standard output that contains the port the plugin
is running on and an optional server key. The port should be one assigned by the operating system so there are no clashes
with other servers. The server key is reserved for use as a bearer token to restrict access to the
plugin from the Pact framework that started it. Ideally, the plugin gRPC server should bind to the loopback interface (127.0.0.1),
but this may not always be possible so if the plugin binds to all interfaces, the server key would provide a security
mechanism to not allow just any process to invoke the plugin methods.

You can see the prototype plugins doing this if you run their executable:

```commandline
$ ~/.pact/plugins/csv-0.0.0/pact-plugin-csv
{"port":35517, "serverKey":"56f7eb63-073b-429c-bff4-6ad336163067"}
```

Refer to the [Plugin drivers](plugin-driver-design.md) for more details.

## Plugin manifest

Each plugin needs to have a manifest file named `pact-plugin.json` in JSON format that describes how the plugin should 
be loaded and any dependencies it requires. The format of the manifest is documented in [Plugin drivers](plugin-driver-design.md). 
This file needs to be installed alongside your plugin executable files. Refer to the [CSV](../plugins/csv/pact-plugin.json) 
and [Protobuf](../plugins/protobuf/pact-plugin.json) manifest files for examples.

The important attribute in the manifest is the `entryPoint`. This is the executable that starts your plugin. The Protobuf
example also has an additional entry for Windows, because it uses batch files to start.

## Installing your plugin

By default, each plugin is installed (along with its manifest) in a directory named `<plugin name>-<version>` in 
the `.pact/plugins` directory in the users home directory. This default can be changed with the `PACT_PLUGIN_DIR`
environment file. `<plugin-name>` is the name of the plugin (corresponding to the name in the manifest) and `<version>`
if the version of the plugin. This way users can have different versions of your plugin installed.

Looking at the `.pact/plugins` on my machine we can see I have the two prototype plugins installed:

```commandline
$ ls -l ~/.pact/plugins/
total 8
drwxrwxr-x 2 ronald ronald 4096 Oct 18 14:09 csv-0.0.0
drwxrwxr-x 6 ronald ronald 4096 Oct 13 15:21 protobuf-0.0.0

$ ls -l ~/.pact/plugins/csv-0.0.0/
total 12376
-rwxrwxr-x 1 ronald ronald 12667032 Oct  6 12:04 pact-plugin-csv
-rw-rw-r-- 1 ronald ronald      237 Oct 18 14:09 pact-plugin.json

$ ls -l ~/.pact/plugins/protobuf-0.0.0/
total 28
drwxr-xr-x 2 ronald ronald  4096 Aug 27 12:40 bin
drwxr-xr-x 2 ronald ronald 12288 Oct 13 12:23 lib
-rw-rw-r-- 1 ronald ronald   352 Oct 11 11:07 pact-plugin.json
drwxrwxr-x 2 ronald ronald  4096 Oct 18 13:33 tmp
```

### If your plugin needs to use disk storage

By default, the plugins should be stateless. They will receive all the required data from Pact framework running the test.
However, if they need to use disk space, they should only write to files within the plugins installed directory. Some
versions of Unix or docker containers may not allow writing to the `/tmp` directory, and you won't know how the Pact tests
are going to be run.

If you look at the Protobuf directory above, you can see a `tmp` directory. This is where the proto file is written to be
passed to the protoc compiler and where the resulting proto descriptor is written. You should also clean up any files
written within the plugin directory.

When the plugin process is started, the current working directory will be set to the plugin's installed directory, so you
can use relative paths to load or write any files. The Protobuf plugin uses the relative path `./tmp` for the proto files.

## Plugin lifecycle

The plugin process will be started when the Pact framework detects that it is needed. This will be when a consumer test
runs that specifies that the plugin must be loaded or when a Pact file that needs the plugin is loaded to be verified. The
plugin driver library will control this. Ideally the plugin process will be kept running for as long as needed, but it may
also be started and stopped for each test. So don't rely on it being a long running process.

