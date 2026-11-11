# Clauddy Dual — macOS floating Claude + Codex HUD

This is a local fork of [Clauddy](https://github.com/renatoaug/claude-usage-monitor). It keeps the animated pixel pet while displaying both providers' session and weekly rate limits without switching tabs.

## Installed app

`~/Applications/Clauddy Dual.app` (bundle ID `dev.aiden.clauddy-dual`).

Launch from Finder or run `open "$HOME/Applications/Clauddy Dual.app"`. The floating compact view is the default for new installs. Use the expand button for Clauddy's original detailed view, and drag the widget to position it.

The card shows **percentage used**, progress bars, and **time remaining until reset**, separately for the 5-hour/session and weekly windows of Claude and Codex. Missing/stale readings show as unavailable rather than 0%. Codex must be enabled under Settings > Codex (already enabled on this Mac). Claude's account usage requires a separate OAuth connection from the app. Codex usage depends on the local Codex app-server/logs, which can sometimes lag or be absent.

## Development and rebuild

Source: `~/Developer/clauddy-dual`. The original project license remains in `LICENSE`.

Rust toolchain and build artifacts are on the external SSD to avoid filling the system drive. In Terminal:

```bash
export RUSTUP_HOME="$HOME/ExternalSSD/DevTools/rustup"
export CARGO_HOME="$HOME/ExternalSSD/DevTools/cargo"
export CARGO_TARGET_DIR="$HOME/ExternalSSD/DevTools/clauddy-target"
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
cd "$HOME/Developer/clauddy-dual/native"
cargo fmt --check
cargo clippy --release --all-targets -- -D warnings
cargo test --release
BUNDLE_ID=dev.aiden.clauddy-dual scripts/bundle.sh
```

The bundle script produces `native/target/Clauddy.app` (where `target` is a symlink to the external build directory); rename its display name and re-sign it if installing a replacement. The custom app intentionally disables updates from upstream so the dual-provider UI isn't overwritten; rebuild from the source fork for updates.

## REA

REA 6.8.0 analyzed the Clauddy JavaScript launcher directory `bin/` successfully (15 nodes, 15 edges). Analysis evidence is in `/tmp/clauddy-rea-evidence.json` while this session's temporary files remain. Clauddy's current UI is Rust/egui rather than JavaScript, so the dual-provider HUD was implemented directly against the open-source native layout and existing provider readers instead of trying to reverse-engineer the Rust binary.
