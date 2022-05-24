#!/usr/bin/env sh

set -e

LATEST_RELEASE=$(curl -L -s -H 'Accept: application/json' https://github.com/pactflow/pact-protobuf-plugin/releases/latest | jq -r '.tag_name')
curl -o /tmp/install-plugin.sh https://github.com/pactflow/pact-protobuf-plugin/releases/download/"${LATEST_RELEASE}"/install-plugin.sh
chmod +x /tmp/install-plugin.sh

case "$(uname -s)" in

   CYGWIN*|MINGW32*|MSYS*|MINGW*)
     export PATH=$PATH:/mnt/C/:\msys64\usr\bin
     ;;

   *)
     ;;
esac


/tmp/install-plugin.sh
