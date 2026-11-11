# QuotaPals: developer notes

The main [README](README.md) covers the experience and public source-install process. This file documents how the fork differs from upstream and how to maintain it.

## What's different

- The compact floating HUD renders each connected provider simultaneously, with separate **5-hour/session** and **weekly** percentages used and reset countdowns for Claude and Codex.
- Claude retains the original orange Clauddy sprite; Codex has a new mint-green robot, generated using [`tools/generate_codex_pet.py`](tools/generate_codex_pet.py).
- Characters react independently to activity and whichever available limit (session or weekly) is highest. Pet-only mode shows both side by side, and the expanded view shows the selected provider's sprite.
- Unknown or expired readings are displayed as unavailable, not as zero.
- The macOS fixed window canvas has been widened to fit the dual HUD.
- The custom bundle identifier is `dev.manynames3.quotapals` so it won't overwrite the official Clauddy app. Upstream auto-update installation is disabled for this ID; changes should be rebuilt from this fork.

## Display modes, holiday companion art, and reset estimates

Three persisted display modes are supported by the existing macOS tray integration: floating widget only, menu-bar popover only, or both side by side. The new Both mode leaves the floating widget visible and makes menu-bar Open focus the existing window instead of repurposing it into a popover.

The Look settings also offer three theme modes: Auto (original calendar-driven seasonal selection), Classic (the chosen default Claude pet and classic Codex robot), and Xmas (Santa-style Claude and matching original Codex Christmas robot). Codex mood sheets for both themes are generated reproducibly with \`tools/generate_codex_pet.py\` followed by \`tools/generate_codex_christmas.py\`. The existing upstream Santa Claude sprite remains credited to Clauddy.

\`format::projected_five_hour_resets\` projects how many nominal five-hour session reset boundaries fit before the weekly reset. It only runs when both timestamps are known and returns no guarantee of quota; actual provider rolling windows may move or not recur without activity. The estimate is deliberately displayed with an approximation sign.

## Auto-continue integration

`native/src/auto_continue.rs` adapts the rate-limit pause detection/recovery workflow from [CodexReset](https://github.com/boyso/codex-reset) (MIT © 2026 BOYSO). This is independently implemented in Rust rather than vendoring the Swift app-server/GUI engine. The automation worker runs alongside the normal poller, but both provider switches default to off; user settings are saved under `autoContinue.claude` and `autoContinue.codex` in the existing `config.json`. The worker requires a recent, valid, below-limit quota reading in a window that opened after the recorded failure. It uses `claude -p --resume` and `codex exec resume` with explicit session IDs, never a shell. The per-session deduplication record is private to `~/.claude-usage-monitor/auto-continue-state.json`.

Only the latest turn's explicit quota error qualifies; unknown failures and completed turns are deliberately skipped. CodexReset must not have its own auto-continue active simultaneously. This feature has unit and visual coverage, but its real-world behavior depends on the installed CLIs and local log formats, so no fully autonomous success guarantee is claimed.

## Architecture and data

The native UI is Rust + `eframe/egui`. It reuses the original upstream account and Codex readers rather than approximating quota using token counts. Claude usage requires authorized access to its reported limits; Codex uses local app-server/session data and may have delayed or missing readings. Local app data is stored in `~/.claude-usage-monitor/`, **outside the Git checkout**. Do not commit that directory or authentication files.

The Codex companion consists of two transparent sprite sheets (4×2 working frames, 4×4 moods). Rebuild them using Pillow:

```bash
python3 -m pip install pillow
python3 tools/generate_codex_pet.py
```

For native macOS builds, install [Rust](https://rustup.rs/) and run `./scripts/install-quotapals-macos.sh` from the repository root. Run the same checks as the [main README](README.md#build-test-and-customize) before publishing changes.

## Publishing policy

The fork's inherited upstream npm/GitHub automatic release workflow is intentionally disabled and saved as `docs/upstream-release-workflow.yml.disabled`. Do not restore it as-is: it is designed to publish the original `clauddy` npm package. This fork currently offers a source install, not public binary releases. `bunx clauddy` and the original root `install.sh` still target the upstream project.

## How REA was used

[REA](https://github.com/morluto/rea) v6.8.0 was used for static analysis of the inherited JavaScript launcher under `bin/`. It **did not** generate or reverse-engineer the Rust UI. The fork's UI and sprites were implemented directly from the open-source Rust project. REA is not bundled with the app and is not a runtime dependency.

## Attribution

Derived from [Clauddy by Renato Augusto](https://github.com/renatoaug/claude-usage-monitor). Preserves the upstream [MIT license](LICENSE) and source attribution.
