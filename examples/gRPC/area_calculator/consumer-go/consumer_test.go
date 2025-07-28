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

func TestCalculateClient(t *testing.T) {
	mockProvider, err := message.NewSynchronousPact(message.Config{
		Consumer: "grpc-consumer-go",
		Provider: "area-calculator-provider",
	})
	assert.NoError(t, err)

	pactlog.SetLogLevel("TRACE")

	dir, _ := filepath.Abs("../proto/area_calculator.proto")

	grpcInteraction := `{
		"pact:proto": "` + filepath.ToSlash(dir) + `",
		"pact:proto-service": "Calculator/calculateMulti",
		"pact:content-type": "application/protobuf",
		"request": {
			"shapes": [
				{
					"rectangle": {
						"length": "matching(number, 3)",
						"width": "matching(number, 4)"
					}
				},
				{
					"square": {
						"edge_length": "matching(number, 3)"
					}
				}
			]
		},		"response": {
			"value": [ "matching(number, 12)", "matching(number, 9)" ],
			"nestedFieldLevel1": [
				{
					"FirstLevel": "matching(equalTo, 'First Level')",
					"InnerLevels": [
						{
							"InnerLevel": "matching(equalTo, 'Inner Level')"
						}
					]
				}
			]
		}
	}`

	// Defined a snew message interaction, and add the plugin config and the contents
	err = mockProvider.
		AddSynchronousMessage("calculate rectangle area request").
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
			resp, err := GetRectangleAndSquareAreaWithNested(fmt.Sprintf("localhost:%d", transport.Port))

			log.Printf("Area for rectangle and square: %v", resp)
			// Assert: check the result
			assert.NoError(t, err)
			assert.Equal(t, float32(12.0), resp.Value[0])
			assert.Equal(t, float32(9.0), resp.Value[1])
			assert.Equal(t, "First Level", resp.NestedFieldLevel1[0].FirstLevel)
			assert.Equal(t, "Inner Level", resp.NestedFieldLevel1[0].InnerLevels[0].InnerLevel)

			return err
		})

	assert.NoError(t, err)
}
