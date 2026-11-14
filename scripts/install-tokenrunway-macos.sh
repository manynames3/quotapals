#!/usr/bin/env bash
# Source installer for TokenRunway. Does not touch the upstream Clauddy app.
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
APP_ID="dev.manynames3.tokenrunway"
BUILD="$ROOT/native/target/TokenRunway.app"
DEST="$HOME/Applications/TokenRunway.app"
LEGACY="$HOME/Applications/QuotaPals.app"

echo "Building TokenRunway from source..."
(cd "$ROOT/native" && BUNDLE_ID="$APP_ID" scripts/bundle.sh)

/usr/libexec/PlistBuddy -c 'Set :CFBundleName TokenRunway' "$BUILD/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName TokenRunway' "$BUILD/Contents/Info.plist"
/usr/bin/codesign --force --sign - --identifier "$APP_ID" "$BUILD"
/usr/bin/codesign --verify "$BUILD"

mkdir -p "$HOME/Applications"
if [[ -e "$DEST" ]] && [[ ! -d "$DEST" ]]; then
  echo "Cannot install: a non-app file already exists at $DEST." >&2
  exit 1
fi
if [[ -d "$DEST" ]] && pgrep -f "^$DEST/Contents/MacOS/tokenrunway$" >/dev/null; then
  echo "Please quit TokenRunway before reinstalling, then run this command again." >&2
  exit 1
fi

# Prevent running both builds against the same settings during migration.
if [[ -d "$LEGACY" ]] && pgrep -f "^$LEGACY/Contents/MacOS/clauddy$" >/dev/null; then
  echo "Please quit QuotaPals before installing TokenRunway." >&2
  exit 1
fi

# Stage the complete signed bundle before touching the previous installation.
STAGE="$HOME/Applications/.TokenRunway-install-$$.app"
trap 'if [[ -d "$STAGE" ]]; then /bin/rm -rf "$STAGE"; fi' EXIT
/usr/bin/ditto "$BUILD" "$STAGE"
/usr/bin/codesign --verify "$STAGE"

if [[ -d "$DEST" ]]; then
  BACKUP="$HOME/Applications/TokenRunway-backup-$(date +%Y%m%d-%H%M%S).app"
  echo "Moving previous TokenRunway app to: $BACKUP"
  /bin/mv "$DEST" "$BACKUP"
fi
/bin/mv "$STAGE" "$DEST"
# Preserve the previous branded installation for a possible rollback.
if [[ -d "$LEGACY" ]]; then
  LEGACY_BACKUP="$HOME/Applications/QuotaPals-backup-$(date +%Y%m%d-%H%M%S).app"
  echo "Backing up previous QuotaPals app: $LEGACY_BACKUP"
  /bin/mv "$LEGACY" "$LEGACY_BACKUP"
fi
echo "Installed: $DEST"
/usr/bin/open "$DEST"
