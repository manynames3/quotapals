# Clauddy Dual: developer notes

The main [README](README.md) covers the experience and public source-install process. This file documents how the fork differs from upstream and how to maintain it.

## What's different

- The compact floating HUD renders each connected provider simultaneously, with separate **5-hour/session** and **weekly** percentages used and reset countdowns for Claude and Codex.
- Claude retains the original orange Clauddy sprite; Codex has a new mint-green robot, generated using [`tools/generate_codex_pet.py`](tools/generate_codex_pet.py).
- Characters react independently to activity and whichever available limit (session or weekly) is highest. Pet-only mode shows both side by side, and the expanded view shows the selected provider's sprite.
- Unknown or expired readings are displayed as unavailable, not as zero.
- The macOS fixed window canvas has been widened to fit the dual HUD.
- The custom bundle identifier is `dev.aiden.clauddy-dual` so it won't overwrite the official Clauddy app. Upstream auto-update installation is disabled for this ID; changes should be rebuilt from this fork.

## Architecture and data

The native UI is Rust + `eframe/egui`. It reuses the original upstream account and Codex readers rather than approximating quota using token counts. Claude usage requires authorized access to its reported limits; Codex uses local app-server/session data and may have delayed or missing readings. Local app data is stored in `~/.claude-usage-monitor/`, **outside the Git checkout**. Do not commit that directory or authentication files.

The Codex companion consists of two transparent sprite sheets (4×2 working frames, 4×4 moods). Rebuild them using Pillow:

```bash
python3 -m pip install pillow
python3 tools/generate_codex_pet.py
```

For native macOS builds, install [Rust](https://rustup.rs/) and run `./scripts/install-dual-macos.sh` from the repository root. Run the same checks as the [main README](README.md#build-test-and-customize) before publishing changes.

## Publishing policy

The fork's inherited upstream npm/GitHub automatic release workflow is intentionally disabled and saved as `docs/upstream-release-workflow.yml.disabled`. Do not restore it as-is: it is designed to publish the original `clauddy` npm package. This fork currently offers a source install, not public binary releases. `bunx clauddy` and the original root `install.sh` still target the upstream project.

## How REA was used

[REA](https://github.com/morluto/rea) v6.8.0 was used for static analysis of the inherited JavaScript launcher under `bin/`. It **did not** generate or reverse-engineer the Rust UI. The fork's UI and sprites were implemented directly from the open-source Rust project. REA is not bundled with the app and is not a runtime dependency.

## Attribution

Derived from [Clauddy by Renato Augusto](https://github.com/renatoaug/claude-usage-monitor). Preserves the upstream [MIT license](LICENSE) and source attribution.
