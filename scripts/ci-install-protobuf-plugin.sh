#!/usr/bin/env sh

set -e

LATEST_RELEASE=$(curl -L -s -H 'Accept: application/json' https://github.com/pactflow/pact-protobuf-plugin/releases/latest | jq -r '.tag_name')

case "$(uname -s)" in

   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     export PATH=$PATH:/c/msys64/usr/bin
     ;;

   *)
     ;;
esac

wget https://github.com/pactflow/pact-protobuf-plugin/releases/download/"${LATEST_RELEASE}"/install-plugin.sh -O /tmp/install-plugin.sh
chmod +x /tmp/install-plugin.sh

/tmp/install-plugin.sh
