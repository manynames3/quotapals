#!/bin/bash
# Build and pack the release artifact for this OS into dist/, stamped with
# the version the release is about to publish (the CI build job runs it on
# each platform's own runner; see docs/design/07-releases.md).
#
#   native/scripts/package.sh 2.0.0
#
#   macOS    dist/TokenRunway-<v>-arm64-mac.zip     TokenRunway.app (dev.manynames3.tokenrunway, ad-hoc signed)
#   Windows  dist/TokenRunway-<v>-win-x64.zip       TokenRunway.exe
#   Linux    dist/TokenRunway-<v>-linux-x64.tar.gz  TokenRunway/tokenrunway + its icon
set -euo pipefail
VERSION="${1:?usage: native/scripts/package.sh <version>}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIST="$ROOT/dist"
mkdir -p "$DIST"
cd "$ROOT/native"

# the version shows in Settings and decides the update check
perl -0pi -e "s/^version = \"[^\"]*\"/version = \"$VERSION\"/m" Cargo.toml

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

case "$(uname -s)" in
  Darwin)
    BUNDLE_ID=dev.manynames3.tokenrunway scripts/bundle.sh
    # ditto keeps the bundle's signature and attributes, which zip would not
    ditto -c -k --keepParent target/TokenRunway.app "$DIST/TokenRunway-$VERSION-arm64-mac.zip"
    ;;
  Linux)
    cargo build --release
    mkdir -p "$STAGE/TokenRunway"
    cp target/release/tokenrunway "$STAGE/TokenRunway/tokenrunway"
    cp assets/icon.png "$STAGE/TokenRunway/tokenrunway.png"
    tar -C "$STAGE" -czf "$DIST/TokenRunway-$VERSION-linux-x64.tar.gz" TokenRunway
    ;;
  MINGW* | MSYS* | CYGWIN*)
    cargo build --release
    cp target/release/tokenrunway.exe "$STAGE/TokenRunway.exe"
    (cd "$STAGE" && 7z a -tzip -bso0 "$DIST/TokenRunway-$VERSION-win-x64.zip" TokenRunway.exe)
    ;;
  *)
    echo "package.sh: no release artifact for $(uname -s)" >&2
    exit 1
    ;;
esac
ls -l "$DIST"
