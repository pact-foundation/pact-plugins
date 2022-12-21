package main

import (
	"fmt"
	"log"
	"path/filepath"
	"testing"

	pactlog "github.com/pact-foundation/pact-go/v2/log"
	message "github.com/pact-foundation/pact-go/v2/message/v4"
	"github.com/stretchr/testify/assert"
)

func TestEnumClient(t *testing.T) {
	mockProvider, err := message.NewSynchronousPact(message.Config{
		Consumer: "grpc-consumer-go",
		Provider: "test-enums",
	})
	assert.NoError(t, err)

	pactlog.SetLogLevel("TRACE")

	dir, _ := filepath.Abs("../proto/test_enum.proto")
	additionalDir, _ := filepath.Abs("../proto2")

	grpcInteraction := `{
		"pact:proto": "` + filepath.ToSlash(dir) + `",
		"pact:proto-service": "Test/GetFeature2",
		"pact:content-type": "application/protobuf",
		"pact:protobuf-config": {
          	"additionalIncludes": ["` + filepath.ToSlash(additionalDir) + `"]
		},
		"request": {
			"latitude": "matching(number, 3)",
			"longitude": "matching(number, 4)"
		},
		"response": {
			"result": "matching(type, 'VALUE_ONE')"
		}
	}`

	// Defined a new message interaction, and add the plugin config and the contents
	err = mockProvider.
		AddSynchronousMessage("get feature with enum").
		UsingPlugin(message.PluginConfig{
			Plugin: "protobuf",
		}).
		WithContents(grpcInteraction, "application/grpc").
		// Start the gRPC mock server
		StartTransport("grpc", "127.0.0.1", nil).
		// Execute the test
		ExecuteTest(t, func(transport message.TransportConfig, m message.SynchronousMessage) error {
			// Execute the gRPC client against the mock server
			log.Println("Mock server is running on ", transport.Port)
			_, err := GetFeature(fmt.Sprintf("localhost:%d", transport.Port))

			// Assert: check the result
			assert.NoError(t, err)
			//assert.Equal(t, float32(12.0), area[0])
			//assert.Equal(t, float32(9.0), area[1])

			return err
		})

	assert.NoError(t, err)
}
