#!/usr/bin/env sh

set -e
set -x

VERSION="1.3.2"

mkdir -p ~/.pact/bin
case "$(uname -s)" in

   Darwin)
     echo '== Installing pact verifier CLI for Mac OS =='
     if [ "$(uname -m)" = "arm64" ]; then
        curl -L -o ~/.pact/bin/pact_verifier_cli.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-macos-aarch64.gz
     else
        curl -L -o ~/.pact/bin/pact_verifier_cli.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-macos-x86_64.gz
     fi
     gunzip -N -f ~/.pact/bin/pact_verifier_cli.gz
     chmod +x ~/.pact/bin/pact-verifier
     ;;

   Linux)
     echo '== Installing pact verifier CLI for Linux =='
     if [ "$(uname -m)" = "aarch64" ]; then
      curl -L -o ~/.pact/bin/pact_verifier_cli.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-linux-aarch64.gz
     else
      curl -L -o ~/.pact/bin/pact_verifier_cli.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-linux-x86_64.gz
     fi
     gunzip -N -f ~/.pact/bin/pact_verifier_cli.gz
     chmod +x ~/.pact/bin/pact-verifier
     ;;

   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     echo '== Installing pact verifier CLI for MS Windows =='
     if [ "$(uname -m)" = "aarch64" ]; then
      curl -L -o ~/.pact/bin/pact_verifier_cli.exe.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-windows-aarch64.exe.gz
     else
      curl -L -o ~/.pact/bin/pact_verifier_cli.exe.gz https://github.com/pact-foundation/pact-reference/releases/download/pact_verifier_cli-v${VERSION}/pact-verifier-windows-x86_64.exe.gz
     fi
     gunzip -N -f ~/.pact/bin/pact_verifier_cli.exe.gz
     chmod +x ~/.pact/bin/pact-verifier.exe
     ;;

   *)
     echo "ERROR: $(uname -s) is not a supported operating system"
     exit 1
     ;;
esac
