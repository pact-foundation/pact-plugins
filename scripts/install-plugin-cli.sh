#!/usr/bin/env sh

set -e
set -x

VERSION="0.3.0"

mkdir -p ~/.pact/bin
case "$(uname -s)" in

   Darwin)
     echo '== Installing plugin CLI for Mac OSX =='
     if [ "$(uname -m)" = "arm64" ]; then
        curl -L -o ~/.pact/bin/pact-plugin.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-macos-aarch64.gz
     else
        curl -L -o ~/.pact/bin/pact-plugin.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-macos-x86_64.gz
     fi
     gunzip -N -f ~/.pact/bin/pact-plugin.gz
     chmod +x ~/.pact/bin/pact-plugin
     ;;

   Linux)
     echo '== Installing plugin CLI for Linux =='
     if [ "$(uname -m)" = "aarch64" ]; then
      curl -L -o ~/.pact/bin/pact-plugin.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-linux-aarch64.gz
     else
      curl -L -o ~/.pact/bin/pact-plugin.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-linux-x86_64.gz
     fi
     gunzip -N -f ~/.pact/bin/pact-plugin.gz
     chmod +x ~/.pact/bin/pact-plugin
     ;;

   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     echo '== Installing plugin CLI for MS Windows =='
     if [ "$(uname -m)" = "aarch64" ]; then
      curl -L -o ~/.pact/bin/pact-plugin.exe.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-windows-aarch64.exe.gz
     else
      curl -L -o ~/.pact/bin/pact-plugin.exe.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-windows-x86_64.exe.gz
     fi
     gunzip -N -f ~/.pact/bin/pact-plugin.exe.gz
     chmod +x ~/.pact/bin/pact-plugin.exe
     ;;

   *)
     echo "ERROR: $(uname -s) is not a supported operating system"
     exit 1
     ;;
esac