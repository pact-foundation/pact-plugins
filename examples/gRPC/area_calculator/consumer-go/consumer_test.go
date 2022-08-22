package main

import (
	"fmt"
	"log"
	"os"
	"testing"

	pactlog "github.com/pact-foundation/pact-go/v2/log"
	message "github.com/pact-foundation/pact-go/v2/message/v4"
	"github.com/stretchr/testify/assert"
)

func TestCalculateClient(t *testing.T) {
	mockProvider, err := message.NewSynchronousPact(message.Config{
		Consumer: "grpc-consumer-go",
		Provider: "area-calculator-provider",
	})
	assert.NoError(t, err)

	pactlog.SetLogLevel("TRACE")

	dir, _ := os.Getwd()
	path := fmt.Sprintf("%s/../proto/area_calculator.proto", dir)

	grpcInteraction := `{
		"pact:proto": "` + path + `",
		"pact:proto-service": "Calculator/calculate",
		"pact:content-type": "application/protobuf",
		"request": {
			"rectangle": {
				"length": "matching(number, 3)",
				"width": "matching(number, 4)"
			}
		},
		"response": {
			"value": "matching(number, 12)"
		}
	}`

	// Defined a new message interaction, and add the plugin config and the contents
	err = mockProvider.
		AddSynchronousMessage("calculate rectangle area request").
		UsingPlugin(message.PluginConfig{
			Plugin:  "protobuf",
			Version: "0.1.10",
		}).
		WithContents(grpcInteraction, "application/grpc").
		// Start the gRPC mock server
		StartTransport("grpc", "127.0.0.1", nil).
		// Execute the test
		ExecuteTest(t, func(transport message.TransportConfig, m message.SynchronousMessage) error {
			// Execute the gRPC client against the mock server
			log.Println("Mock server is running on ", transport.Port)
			area, err := GetSquareArea(fmt.Sprintf("localhost:%d", transport.Port))

			// Assert: check the result
			assert.NoError(t, err)
			var f float32
			f = 12
			assert.Equal(t, f, area)

			return err
		})

	assert.NoError(t, err)
}
