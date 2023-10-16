

# Verifier CLI

verifier_cli_install_cargo:
	cargo install pact_verifier_cli

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

## Rust

consumer_test_rust:
	cd consumer-rust && \
		cargo test

## JVM - Gradle

consumer_test_jvm:
	cd consumer-jvm && \
		./gradlew clean test

provider_run_jvm:
	cd provider-jvm && \
		./gradlew run
provider_test_jvm:
	cd provider-jvm && \
		./gradlew clean test
provider_test_jvm_cli:
	cd provider-jvm && \
		pact_verifier_cli -f ../consumer-jvm/build/pacts/grpc-consumer-jvm-area-calculator-provider.json \
		--transport grpc -p $$PORT

## JVM - Maven

consumer_test_maven:
	cd consumer-maven && \
		mvn test
pacts_move_maven:
	cd provider-maven && \
	mkdir -p src/test/resources/pacts && \
	cp ../consumer-maven/target/pacts/*.json src/test/resources/pacts
provider_test_maven:
	cd provider-maven && \
		mvn clean test