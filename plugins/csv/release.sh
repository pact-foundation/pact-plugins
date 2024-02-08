#!/bin/bash

if [ $# -lt 1 ]
then
    echo "Usage : $0 <Linux|Windows|macOS>"
    exit
fi

echo Building Release for "$1"

cargo clean
mkdir -p release_artifacts

case "$1" in
  Linux)    echo "Building for Linux"
            docker run --rm --user "$(id -u)":"$(id -g)" -v "$(pwd):/workspace" -w /workspace -t pactfoundation/rust-musl-build -c 'cargo build --release'
            gzip -c target/release/pact-csv-plugin > release_artifacts/pact-csv-plugin-linux-x86_64.gz
            openssl dgst -sha256 -r release_artifacts/pact-csv-plugin-linux-x86_64.gz > release_artifacts/pact-csv-plugin-linux-x86_64.gz.sha256
            cp pact-plugin.json release_artifacts/
            cargo install cross
            cross build --target aarch64-unknown-linux-gnu --release
            gzip -c target/aarch64-unknown-linux-gnu/release/pact-csv-plugin > release_artifacts/pact-csv-plugin-linux-aarch64.gz
            openssl dgst -sha256 -r release_artifacts/pact-csv-plugin-linux-aarch64.gz > release_artifacts/pact-csv-plugin-linux-aarch64.gz.sha256
            ;;
  Windows)  echo  "Building for Windows"
            cargo build --release
            gzip -c target/release/pact-csv-plugin.exe > release_artifacts/pact-csv-plugin-windows-x86_64.exe.gz
            openssl dgst -sha256 -r release_artifacts/pact-csv-plugin-windows-x86_64.exe.gz > release_artifacts/pact-csv-plugin-windows-x86_64.exe.gz.sha256
            ;;
  macOS)    echo  "Building for OSX"
            cargo build --release
            gzip -c target/release/pact-csv-plugin > release_artifacts/pact-csv-plugin-osx-x86_64.gz
            openssl dgst -sha256 -r release_artifacts/pact-csv-plugin-osx-x86_64.gz > release_artifacts/pact-csv-plugin-osx-x86_64.gz.sha256

            # M1
            export SDKROOT=$(xcrun -sdk macosx11.1 --show-sdk-path)
            export MACOSX_DEPLOYMENT_TARGET=$(xcrun -sdk macosx11.1 --show-sdk-platform-version)
            cargo build --target aarch64-apple-darwin --release

            gzip -c target/aarch64-apple-darwin/release/pact-csv-plugin > release_artifacts/pact-csv-plugin-osx-aarch64.gz
            openssl dgst -sha256 -r release_artifacts/pact-csv-plugin-osx-aarch64.gz > release_artifacts/pact-csv-plugin-osx-aarch64.gz.sha256
            ;;
  *)        echo "$1 is not a recognised OS"
            exit 1
            ;;
esac
