#!/usr/bin/env sh

set -e

cargo install pact-plugin-cli
pact-plugin-cli install https://github.com/pactflow/pact-protobuf-plugin/releases/latest
