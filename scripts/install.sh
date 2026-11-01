#!/bin/sh
set -e
cd "$(dirname "$0")/.."
./scripts/bundle.sh
DEST=/Applications
[ -w "$DEST" ] || DEST="$HOME/Applications"
mkdir -p "$DEST"
rm -rf "$DEST/riff.app"
cp -R target/riff.app "$DEST/riff.app"
echo "installed $DEST/riff.app"
