#!/usr/bin/env sh
#
# Installs the JWT Lua example plugin into the local plugin directory so it can be
# discovered by the driver. There is nothing to compile (it's plain Lua files), so this
# just copies the plugin directory into place.

set -e

VERSION="0.0.0"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLUGIN_DIR="${PACT_PLUGIN_DIR:-$HOME/.pact/plugins}"
DEST="$PLUGIN_DIR/jwt-$VERSION"

mkdir -p "$DEST"
cp "$SCRIPT_DIR"/*.lua "$SCRIPT_DIR"/pact-plugin.json "$DEST"/

echo "Installed the jwt plugin into $DEST"
