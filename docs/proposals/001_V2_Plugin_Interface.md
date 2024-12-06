# V2 Plugin Interface (Draft)

Discussion for this proposal: https://github.com/pact-foundation/pact-plugins/discussions/83

## Summary

The current V1 plugin interface has a number of issues, and was released as an MVP to get the plugin architecture 
going. It does not completely implement the plugin design as laid in the design and concept documents. 

## Motivation

There are a number of issues with using and/or authoring plugins. We are motivated to address them.

## Details

There are a number of issues with how plugins work, and to date there are really only been two plugins written: 
gRPC/Protobuf and Avro. I think this is a symptom of the complexity of plugins. 

Here are some comments taken from a slack discussion:

> Some of the current improvements we’ve identified: ability to mix and match plugins of different types, 
> using plugins at the field or element level (e.g. for JWT headers or keys in payloads), ability for plugin authors 
> to identify matching interactions (see [this issue](https://github.com/pact-foundation/pact-plugins/issues/35)), 
> gRPC vs other approaches debugging plugin issues and more ...

Issues raised on GitHub:
- https://github.com/pact-foundation/pact-plugins/issues/35
- https://github.com/pact-foundation/pact-plugins/issues/37
- https://github.com/pact-foundation/pact-plugins/issues/41

So I think there are 4 main things to address:
1. Deal with the logging issues.
2. Create a V2 plugin interface that completes the original designs and addresses some of the issues in the V1 interface.
3. Add support for plugins that interact at the field level and provide matching logic.
4. Allow plugins to be able to use other plugins.

## Technical details

### Deal with the logging issues.
The original comment was to deal with logging and debugging. Logging issues could be addressed, but debugging will be 
an issue as the plugins run as a separate process and this will not be addressed with this proposal.

Currently, all the plugin standard output is captured and logged via the plugin driver, so it ends up in the standard
logging used by the Pact framework running the tests. While this means that all the logs end up in one place, at trace
level, it can be very verbose and not very useful. Also, if the Rust driver is used (calls via FFI use this driver,
so this is all language implementations except JVM), and the plugin is written in Rust (gRPC is), you get Rust logs
from both sides and it is hard determine which is which.

The proposal is to not forward plugins output on to the running Pact implementation, but write them to a timestamped 
file in the plugin directory. The gRPC already does this (its logs go both to standard out and a file).

### Create a V2 plugin interface that completes the original designs and addresses some of the issues in the V1 interface

This is split into 2 parts, dealing with the issues in the V1 interface and addressing the missing parts from the 
original design.

#### Issues in the V1 interface

The following issues have been observed in the current plugin interface. These need to be addressed, but will require 
changes to the interface so will have to go into a V2 version.

1. VerifyInteractionRequest passes the Pact through as JSON
This requires the plugin to be able to parse Pact JSON, so needs to have a Pact implementation as a dependency. It also
then has to find the correct interaction to verify. The interface should just pass through the relevant data required (
the interaction as well as any required metadata).

#### Missing features

The following features from the original design have not been implemented:

##### Allow plugins to match specific data
This is a mechanism to have plugins add new matchers and generators. The plugin catalog allows defining a MATCHER entry,
and all the core Pact matchers are exposed in the catalogue, but there is no current way to call out to a plugin to
do this. This proposal is to add two new RPC calls that a plugin can implement, and then update to the Pact frameworks
to call out to the plugin when the matcher/generator is required.

```protobuf
message MatchDataRequest {
  // Catalogue entry for the matcher
  CatalogueEntry entry = 1;
  // Expected data from the Pact
  google.protobuf.Value expectedData = 2;
  // Actual data received
  google.protobuf.Value actualData = 3;
}

message MatchDataResult {
  // Any mismatches that occurred
  repeated Mismatch mismatches = 1;
}

message Mismatch {
  // Description of the mismatch
  string mismatch = 1;
  // Path to the item that was matched. This is the value as per the documented Pact matching rule expressions.
  string path = 2;
}

message GenerateDataRequest {
  // Catalogue entry for the matcher
  CatalogueEntry entry = 1;
  // Expected data from the Pact
  google.protobuf.Value expectedData = 2;
}

message GenerateDataResult {
  // Generated data
  google.protobuf.Value generatedData = 1;
}

service PactPluginV2 {
  rpc MatchData(MatchDataRequest) returns (MatchDataResult);
  rpc GenerateData(GenerateDataRequest) returns (GenerateDataResult);
}
```

Issues with this approach:

1. This can only work for data that can be represented as JSON. Binary data is not supported.

##### Capability for plugins to use the functionality from the calling framework
This will allow plugins to not need to use a Pact framework as a dependency, but use the functionality from the calling
framework. This will also allow plugins to use functionality provided by other plugins.

The original design had the plugin driver acting like a central hub (see this [sequence diagram](https://github.com/pact-foundation/pact-plugins/blob/main/docs/pact-plugin.png)).
Each plugin could call back in have things resolved by either another plugin or the core framework.

To implement this, gRPC supports bi-directional streaming connections. So instead of using a unary call, the RPC 
service methods will be updated to allow a sequence of messages to resolve the original request. A high-level example of
a match JSON body request would go something like (assuming JSON support is from a plugin):

1. Driver sends Start(MatchRequest, ID) message to the plugin with the data and a correlation ID.
2. Plugin parses the JSON and starts the matching process.
3. Plugin responds with Continue(MatchDataRequest, ID, ID1) to get a matching rule applied to an item.
4. Driver calls the Pact framework to resolve the matching rule. 
5. Driver responds with Done(MatchDataResult, ID, ID1) to the plugin.
6. ... This can continue back and forth until
7. Plugin responds with Done(MatchDataResponse, ID).

Issues with this approach:

1. This makes the calls asynchronous. If a message is lost for some reason (i.e. driver calls plugin 1, plugin 1 calls 
   back for something, driver calls plugin 2, plugin 2 does not respond, the call will never resolve). gRPC 
   bi-directional streams allow the client to impose a deadline so that a timeout error is raised if things are not
   resolved in a certain period of time.
2. This can introduce cyclic dependency issues. The current drivers are only dependent on the models from the core framework,
   but will now need to be dependent on the core framework, while the core framework is also dependent on the driver.
