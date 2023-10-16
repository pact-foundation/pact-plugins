

# GoLang
consumer_build_go:
	cd consumer-go && \
		go mod tidy && \
		go mod vendor
provider_build_go:
	cd provider-go && \
		go mod tidy && \
		go mod vendor
consumer_test_go:
	cd consumer-go && go test
provider_run_go:
	cd provider-go && go run provider.go
provider_test_go:
	cd provider-go && go test provider_test.go
provider_test_go_cli:
	cd provider-go && \
		pact_verifier_cli -f ../consumer-go/pacts/grpc-consumer-go-area-calculator-provider.json \
		--transport grpc -p $$PORT