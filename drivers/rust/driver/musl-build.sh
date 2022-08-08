#!/bin/bash

set -e

apk add protobuf protobuf-dev
cargo install pact-plugin-cli
pact-plugin-cli install https://github.com/pactflow/pact-protobuf-plugin/releases/latest
# pact-plugin-cli install https://github.com/pact-foundation/pact-plugins/releases/tag/csv-plugin-0.0.2
cargo build
cargo test
