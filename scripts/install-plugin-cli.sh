#!/usr/bin/env sh

set -e
set -x

VERSION="0.0.3"

mkdir -p ~/bin
case "$(uname -s)" in

   Darwin)
     echo '== Installing plugin CLI for Mac OSX =='
     if [ "$(uname -m)" == "arm64" ]; then
        curl -L -o ~/bin/pact-plugin-cli.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-cli-osx-aarch64.gz
     else
        curl -L -o ~/bin/pact-plugin-cli.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-cli-osx-x86_64.gz
     fi
     gunzip -N -f ~/bin/pact-plugin-cli.gz
     chmod +x ~/bin/pact-plugin-cli
     ;;

   Linux)
     echo '== Installing plugin CLI for Linux =='
     curl -L -o ~/bin/pact-plugin-cli.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-cli-linux-x86_64.gz
     gunzip -N -f ~/bin/pact-plugin-cli.gz
     chmod +x ~/bin/pact-plugin-cli
     ;;

   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     echo '== Installing plugin CLI for MS Windows =='
     curl -L -o ~/bin/pact-plugin-cli.exe.gz https://github.com/pact-foundation/pact-plugins/releases/download/pact-plugin-cli-v${VERSION}/pact-plugin-cli-windows-x86_64.exe.gz
     gunzip -N -f ~/bin/pact-plugin-cli.exe.gz
     chmod +x ~/bin/pact-plugin-cli.exe
     ;;

   *)
     echo "ERROR: $(uname -s) is not a supported operating system"
     exit 1
     ;;
esac
