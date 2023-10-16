package main

import (
	"fmt"

	"os"
	"path/filepath"
	"testing"
	"log"
	"net"

	ac "area_calculator/provider/io.pact/area_calculator"
	area_provider "area_calculator/provider"
	l "github.com/pact-foundation/pact-go/v2/log"
	"github.com/pact-foundation/pact-go/v2/provider"
	"github.com/stretchr/testify/assert"
	"google.golang.org/grpc"
)

var dir, _ = os.Getwd()

func TestGrpcProvider(t *testing.T) {
	go startProvider()
	l.SetLogLevel("INFO")

	verifier := provider.NewVerifier()

	err := verifier.VerifyProvider(t, provider.VerifyRequest{
		ProviderBaseURL: "http://localhost:8222",
		Transports: []provider.Transport{
			provider.Transport{
				Protocol: "grpc",
				Port:     8222,
			},
		},
		Provider: "area-calculator-provider",
		PactFiles: []string{
			filepath.ToSlash(fmt.Sprintf("%s/../consumer-go/pacts/grpc-consumer-go-area-calculator-provider.json", dir)),
		},
	})

	assert.NoError(t, err)
}

func startProvider() {
	lis, err := net.Listen("tcp", fmt.Sprintf("localhost:%d", 8222))
	if err != nil {
		log.Fatalf("failed to listen: %v", err)
	}
	var opts []grpc.ServerOption
	grpcServer := grpc.NewServer(opts...)
	ac.RegisterCalculatorServer(grpcServer, area_provider.NewServer())
	grpcServer.Serve(lis)
}