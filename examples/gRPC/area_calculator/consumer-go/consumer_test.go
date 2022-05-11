package main

import (
	"fmt"
	"testing"

	. "github.com/pact-foundation/pact-go/v2/sugar"
	"github.com/stretchr/testify/assert"
)

func TestCalculateClient(t *testing.T) {
	mockProvider, err := NewV2Pact(MockHTTPProviderConfig{
		Consumer: "grpc-consumer-go",
		Provider: "area-calculator-provider",
	})
	assert.NoError(t, err)

	// Arrange: Setup our expected interactions
	//mockProvider.
	//	AddInteraction().
	//	Given("A user with ID 10 exists").
	//	UponReceiving("A request for User 10").
	//	WithRequest("GET", S("/user/10")).
	//	WillRespondWith(200).
	//	WithBodyMatch(&User{})

	// Act: test our API client behaves correctly
	err = mockProvider.ExecuteTest(func(config MockServerConfig) error {
		// Execute the gRPC client against the mock server
		area, err := GetSquareArea(fmt.Sprintf("%s:%d", config.Host, config.Port))

		// Assert: check the result
		assert.NoError(t, err)
		assert.Equal(t, 9, area)

		return err
	})
	assert.NoError(t, err)
}
