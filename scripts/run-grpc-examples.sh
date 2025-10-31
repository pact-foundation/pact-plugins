#!/bin/bash

set -e

export pact_do_not_track=true

echo '==== RUNNING consumer-jvm'
cd consumer-jvm
./gradlew check

echo '==== RUNNING consumer-maven'
cd ../consumer-maven
mvn verify

echo '==== RUNNING consumer-rust'
cd ../consumer-rust
cargo test

echo '==== RUNNING consumer-go'
cd ../consumer-go

case "$(uname -s)" in
   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     export CGO_LDFLAGS="-g -O2 -L$USERPROFILE\\.pact"
     ;;
esac

go test -c
LOG_LEVEL=info ./consumer.test

echo '==== RUNNING provider-jvm'
cd ../provider-jvm
cp ../consumer-jvm/build/pacts/* server/src/test/resources/pacts
cp ../consumer-rust/target/pacts/* server/src/test/resources/pacts
cp ../consumer-go/pacts/* server/src/test/resources/pacts
./gradlew check

echo '==== RUNNING provider-go'
cd ../provider-go
go build provider.go
nohup ./provider > provider.go.out 2>&1 &
PID=$!
trap "kill $PID" EXIT
sleep 1
ls -la
PROVIDER_PORT=$(cat provider.go.out | cut -f4 -d:)
~/.pact/bin/pact_verifier_cli -f ../consumer-jvm/build/pacts/grpc-consumer-jvm-area-calculator-provider.json\
  -f ../consumer-rust/target/pacts/grpc-consumer-rust-area-calculator-provider.json\
  -f ../consumer-go/pacts/grpc-consumer-go-area-calculator-provider.json\
  -p "$PROVIDER_PORT"
