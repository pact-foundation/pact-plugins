#!/bin/bash -eu
set -e
set -u

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
            cargo install cross@0.2.5

            echo -- Build the x86_64 release artifacts --
            cargo clean
            cross build --target x86_64-unknown-linux-gnu --release
            gzip -c target/x86_64-unknown-linux-gnu/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-linux-x86_64.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-linux-x86_64.gz > release_artifacts/pact-plugin-cli-linux-x86_64.gz.sha256

            echo -- Build the aarch64 release artifacts --
            cargo clean
            cross build --target aarch64-unknown-linux-gnu --release
            gzip -c target/aarch64-unknown-linux-gnu/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-linux-aarch64.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-linux-aarch64.gz > release_artifacts/pact-plugin-cli-linux-aarch64.gz.sha256

            echo -- Build the musl x86_64 release artifacts --
            cargo clean
            cross build --release --target=x86_64-unknown-linux-musl
            gzip -c target/x86_64-unknown-linux-musl/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-linux-x86_64-musl.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-linux-x86_64-musl.gz > release_artifacts/pact-plugin-cli-linux-x86_64-musl.gz.sha256

            echo -- Build the musl aarch64 release artifacts --
            cargo clean
            cross build --release --target=aarch64-unknown-linux-musl
            gzip -c target/aarch64-unknown-linux-musl/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-linux-aarch64-musl.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-linux-aarch64-musl.gz > release_artifacts/pact-plugin-cli-linux-aarch64-musl.gz.sha256

            ;;
  Windows)  echo  "Building for Windows"
            cargo build --release
            gzip -c target/release/pact-plugin-cli.exe > release_artifacts/pact-plugin-cli-windows-x86_64.exe.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-windows-x86_64.exe.gz > release_artifacts/pact-plugin-cli-windows-x86_64.exe.gz.sha256
            ;;
  macOS)    echo  "Building for OSX"
            cargo build --release
            gzip -c target/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-osx-x86_64.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-osx-x86_64.gz > release_artifacts/pact-plugin-cli-osx-x86_64.gz.sha256

            # M1
            export SDKROOT=$(xcrun -sdk macosx11.1 --show-sdk-path)
            export MACOSX_DEPLOYMENT_TARGET=$(xcrun -sdk macosx11.1 --show-sdk-platform-version)
            cargo build --target aarch64-apple-darwin --release

            gzip -c target/aarch64-apple-darwin/release/pact-plugin-cli > release_artifacts/pact-plugin-cli-osx-aarch64.gz
            openssl dgst -sha256 -r release_artifacts/pact-plugin-cli-osx-aarch64.gz > release_artifacts/pact-plugin-cli-osx-aarch64.gz.sha256
            ;;
  *)        echo "$1 is not a recognised OS"
            exit 1
            ;;
esac
