#!/usr/bin/env bash
# Source installer for QuotaPals. Does not touch the upstream Clauddy app.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This installer currently supports macOS only." >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust/Cargo is required. Install it from https://rustup.rs/ first." >&2
  exit 1
fi
if ! command -v codesign >/dev/null 2>&1; then
  echo "Install Xcode Command Line Tools: xcode-select --install" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_ID="dev.manynames3.quotapals"
BUILD="$ROOT/native/target/Clauddy.app"
DEST="$HOME/Applications/QuotaPals.app"

echo "Building QuotaPals from source..."
(cd "$ROOT/native" && BUNDLE_ID="$APP_ID" scripts/bundle.sh)

/usr/libexec/PlistBuddy -c 'Set :CFBundleName QuotaPals' "$BUILD/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName QuotaPals' "$BUILD/Contents/Info.plist"
/usr/bin/codesign --force --sign - --identifier "$APP_ID" "$BUILD"
/usr/bin/codesign --verify "$BUILD"

mkdir -p "$HOME/Applications"
if [[ -e "$DEST" ]] && [[ ! -d "$DEST" ]]; then
  echo "Cannot install: a non-app file already exists at $DEST." >&2
  exit 1
fi
if [[ -d "$DEST" ]] && pgrep -f "^$DEST/Contents/MacOS/clauddy$" >/dev/null; then
  echo "Please quit QuotaPals before reinstalling, then run this command again." >&2
  exit 1
fi

# Stage the complete signed bundle before touching the previous installation.
STAGE="$HOME/Applications/.QuotaPals-install-$$.app"
trap 'if [[ -d "$STAGE" ]]; then /bin/rm -rf "$STAGE"; fi' EXIT
/usr/bin/ditto "$BUILD" "$STAGE"
/usr/bin/codesign --verify "$STAGE"

if [[ -d "$DEST" ]]; then
  BACKUP="$HOME/Applications/QuotaPals-backup-$(date +%Y%m%d-%H%M%S).app"
  echo "Moving previous QuotaPals app to: $BACKUP"
  /bin/mv "$DEST" "$BACKUP"
fi
/bin/mv "$STAGE" "$DEST"
echo "Installed: $DEST"
/usr/bin/open "$DEST"
