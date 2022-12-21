#!/bin/bash

if [ $# -lt 1 ]
then
    echo "Usage : $0 <Linux|Windows|macOS>"
    exit
fi

echo Building Release for "$1"

cargo clean
mkdir -p target/artifacts/

case "$1" in
  Linux)    echo "Building for Linux"
            docker run --rm --user "$(id -u)":"$(id -g)" -v "$(pwd):/workspace" -w /workspace -t pactfoundation/rust-musl-build -c 'cargo build --release'
            gzip -c target/release/pact-plugin-cli > target/artifacts/pact-plugin-cli-linux-x86_64.gz
            openssl dgst -sha256 -r target/artifacts/pact-plugin-cli-linux-x86_64.gz > target/artifacts/pact-plugin-cli-linux-x86_64.gz.sha256

            # Build aarch64
            cargo install cross
            cross build --target aarch64-unknown-linux-gnu --release
            gzip -c target/aarch64-unknown-linux-gnu/release/pact-plugin-cli > target/artifacts/pact-plugin-cli-linux-aarch64.gz
            openssl dgst -sha256 -r target/artifacts/pact-plugin-cli-linux-aarch64.gz > target/artifacts/pact-plugin-cli-linux-aarch64.gz.sha256
            ;;
  Windows)  echo  "Building for Windows"
            cargo build --release
            gzip -c target/release/pact-plugin-cli.exe > target/artifacts/pact-plugin-cli-windows-x86_64.exe.gz
            openssl dgst -sha256 -r target/artifacts/pact-plugin-cli-windows-x86_64.exe.gz > target/artifacts/pact-plugin-cli-windows-x86_64.exe.gz.sha256
            ;;
  macOS)    echo  "Building for OSX"
            cargo build --release
            gzip -c target/release/pact-plugin-cli > target/artifacts/pact-plugin-cli-osx-x86_64.gz
            openssl dgst -sha256 -r target/artifacts/pact-plugin-cli-osx-x86_64.gz > target/artifacts/pact-plugin-cli-osx-x86_64.gz.sha256

            # M1
            export SDKROOT=$(xcrun -sdk macosx11.1 --show-sdk-path)
            export MACOSX_DEPLOYMENT_TARGET=$(xcrun -sdk macosx11.1 --show-sdk-platform-version)
            cargo build --target aarch64-apple-darwin --release

            gzip -c target/aarch64-apple-darwin/release/pact-plugin-cli > target/artifacts/pact-plugin-cli-osx-aarch64.gz
            openssl dgst -sha256 -r target/artifacts/pact-plugin-cli-osx-aarch64.gz > target/artifacts/pact-plugin-cli-osx-aarch64.gz.sha256
            ;;
  *)        echo "$1 is not a recognised OS"
            exit 1
            ;;
esac
