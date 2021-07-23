# Protocol Plugin Implementations

One of the features a plugin can provide is a protocol implementation. For these, the plugin needs to provide
the following features.

## Required Features

### Mock server

### Provider verifier

## Example interaction from a consumer test

_High Level Summary_

1. User is responsible for starting the plugin following plugin specific documentation. The plugin must start an administration HTTP server, which will be used by the framework to communicate instructions for each Test Session
2. Pact is given plugin specific configuration - including the administration API details - which is then sent to the administration server to initialise a new test session. This step should result in a new service being started for use by the test code (e.g. a TCP socket or a protobuf server) and a unique session ID returned. Each session must be thread safe and isolated from any other sessions
3. The Pact framework will maintain the details of the TestSession - including interactions, failures, logs  etc.
4. The calling code is now able to add Interactions to the plugin, which are stored by the framework and registered with the plugin. The plugin is responsible for defining what an Interaction looks like and how it should be passed in for its specific combination of protocol, payload, transport and interaction type.
5. During Test Execution, the calling code communicates directly to the Mock Service provided by the plugin. The Mock Service is responsible for handling the request, comparing the request against the registered interactions, and returning a suitable response. It must keep track of the interactions that were matched during the test session.
6. After each individual Test Execution, verify() is called to see if the expected Interactions matched the actual Interactions. Any mismatches are retrieved from the plugin and returned to the caller.
7. If the Test Session was successful, write_pact() is called which will write out the actual pact file.
8. The plugin is shutdown by the User code.

_Consumer Sequence Diagram_
![pact_consumer_plugin_sequence](https://user-images.githubusercontent.com/53900/103766860-ab47c280-5073-11eb-9d6b-f1c4a3a27232.png)

_Example consumer test_

Here is an example for a raw "hello world" TCP provider. It should respond with "world!" if "hello" is sent:

```golang
func TestPluginPact(t *testing.T) {
	// Start plugin
	go startTCPPlugin()

	provider, err := v3.NewPluginProvider(v3.PluginProviderConfig{
		Consumer: "V3MessageConsumer",
		Provider: "V3MessageProvider", // must be different to the HTTP one, can't mix both interaction styles
		Port:     4444,                // Communication port to the provider
	})

	if err != nil {
		t.Fatal(err)
	}

	type tcpInteraction struct {
		Message   string `json:"message"`   // consumer request
		Response  string `json:"response"`  // expected response
		Delimeter string `json:"delimeter"` // how to determine message boundary
	}

	// Plugin providers could create language specific interfaces that except well defined types
	// The raw plugin interface accepts an interface{}
	provider.AddInteraction(tcpInteraction{
		Message:   "hello",
		Response:  "world!",
		Delimeter: "\r\n",
	})

	// Execute pact test
	if err := provider.ExecuteTest(tcpHelloWorldTest); err != nil {
		log.Fatalf("Error on Verify: %v", err)
	}
}
```

## Example interaction for verifying a provider

_High Level Summary_

1. User is responsible for starting the plugin following plugin specific documentation. The plugin must start an administration HTTP server, which will be used by the framework to communicate instructions for each Test Session
2. Pact is given plugin specific configuration - including the administration API details - which is then sent to the administration server to initialise a new provider Test Session. 3. The user starts the Provider Service, and runs the verify() command
4. Pact fetches the pact files (e.g. from the broker), including the pacts for verification details if configured, and stores this information. 5. For each pact, the framework will be responsible for configuring provider states, and sending each interaction from the pact file to the plugin. The plugin will then perform the plugin-specific interaction, communicating with the Provider Service and returning any mismatches to the framework. This process repeats for all interactions in all pacts. 6. The Pact framework will maintain the details of the TestSession - including pacts, interaction failures, pending status, logs  etc.
7. Pact calculates the verification status for the test session, and optionally publishes verification results back to a Broker 8. The Pact client library then conveys the verification status, and the User terminates all process.s

_Provider Sequence Diagram_
![pact_provider_plugin_sequence](https://user-images.githubusercontent.com/53900/103849702-872ec480-50f9-11eb-93ff-28d1fbfa5cd6.png)


_Example provider test_

Here is an example for a raw "hello world" TCP provider test.

```golang
func TestV3PluginProvider(t *testing.T) {
	go startTCPPlugin()
	go startTCPProvider()

	provider, err := v3.NewPluginProvider(v3.PluginProviderConfig{
		Provider: "V3MessageProvider",
		Port:     4444, // Communication port to the provider
	})

	verifier := v3.HTTPVerifier{
		PluginConfig: provider,
	}

	if err != nil {
		t.Fatal(err)
	}

	// Verify the Provider with local Pact Files
	err = verifier.VerifyPluginProvider(t, v3.VerifyPluginRequest{
		BrokerURL:      os.Getenv("PACT_BROKER_URL"),
		BrokerToken:    os.Getenv("PACT_BROKER_TOKEN"),
		BrokerUsername: os.Getenv("PACT_BROKER_USERNAME"),
		BrokerPassword: os.Getenv("PACT_BROKER_PASSWORD"),
		PublishVerificationResults: true,
		ProviderVersion:            "1.0.0",		
		StateHandlers: v3.StateHandlers{
			"world exists": func(s v3.ProviderStateV3) error {
				// ... do something
				return nil
			},
		},
	})

	assert.NoError(t, err)
}
```
