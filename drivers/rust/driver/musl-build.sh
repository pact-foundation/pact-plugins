#!/bin/bash

set -e
set -x

apk add protobuf protobuf-dev
wget https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v0.3.0/pact-plugin-linux-x86_64.gz
gunzip pact-plugin-linux-x86_64.gz
chmod +x pact-plugin-linux-x86_64
./pact-plugin-linux-x86_64 -y install https://github.com/pactflow/pact-protobuf-plugin/releases/latest
cargo build
cargo test
