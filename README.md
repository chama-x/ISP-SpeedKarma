## ISP‑SpeedKarma

Make your ISP behave. SpeedKarma learns your network’s rhythms, then nudges traffic like a pro — so your real stuff flies when it matters.

### What this is (in plain human)
- **Learns**: watches your throughput patterns and builds a baseline
- **Optimizes**: when there’s signal, it boosts your effective bandwidth without being noisy
- **Stays stealthy**: can disguise app traffic to look like speedtest flows when needed
- **Lives in your tray**: fast toggles, gentle notifications, no drama

### Screenshot vibes
Drop your screenshots here once you’re ready.

![Main window placeholder](docs/images/screenshot-main.png)

![Tray menu placeholder](docs/images/screenshot-tray.png)


## Highlights
- **Apple‑style tray UI**: Minimal menu with a status line and one‑click toggle
- **Smart baseline**: learns before it optimizes — no placebo switches
- **Speedtest runner**: parallelized up/down tests with progress events
- **Booster/keeper**: burst pacing to maintain smoothness under caps
- **Disguise mode**: optional headers/flows that resemble speedtests
- **Tauri app**: tiny footprint, native feel, cross‑platform bundles (dmg/msi)


## How it works (short version)
1. App starts in Learning. It collects a few sessions of normal bandwidth.
2. When confidence is good, Optimization becomes available.
3. With Optimization on, SpeedKarma manages pacing, routes, and bursts.
4. You can run a full‑bandwidth Speedtest from the UI to sanity‑check.


## Install and run
Prereqs:
- Rust (stable) and Cargo
- Tauri toolchain for your OS (Xcode CLTs on macOS; MSVC on Windows)
- Tauri CLI: `cargo install tauri-cli`

Dev run:
```bash
cargo tauri dev
```

Build app bundle:
```bash
cargo tauri build
```

### macOS signing & notarization (distribution)
To avoid the “App is damaged and can’t be opened” warning on other Macs, distribute a signed and notarized build:

1. Prereqs: Developer ID cert, Apple ID with app-specific password, Team ID.
2. Ensure `tauri.conf.json` has hardened runtime and entitlements configured (already added).
3. Set environment variables before building:
```bash
export APPLE_ID="your@appleid.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="YOURTEAMID"
export TAURI_PRIVATE_KEY="<optional if using TAURI code signing>"
```
4. Build signed & submit for notarization (Tauri CLI will use your env):
```bash
cargo tauri build
```
5. After build, staple the notarization ticket (if not already stapled):
```bash
xcrun stapler staple "src-tauri/target/release/bundle/macos/ISP-SpeedKarma.app"
```
Distribute the resulting `.dmg` under `src-tauri/target/release/bundle/dmg/`.


## The UI in 10 seconds
- Toggle tile: enable/disable optimization. It stays locked while we’re learning.
- Insights: live confidence and improvement when available.
- Speedtest: fires a multi‑stream test; progress shows in the tile.
- Disguise: optional — mimics speedtest‑style traffic.
- Tray: left‑click for the popover; native menu is minimal (Enable/Disable, Run Speedtest, Quit).
- Popover: Tesla/SpaceX‑inspired panel with a Mode selector (Auto/Enabled/Disabled).


## Attitude and respect
This app is assertive, not aggressive. It optimizes within your constraints, avoids noisy antics, and tells you clearly what it’s doing.


## FAQ
- “Why is Optimization disabled?”
  Because we’re still Learning — we don’t flip the switch until there’s enough baseline.

- “Does this hide me from my ISP?”
  No cloaking claims here. Disguise mode just makes traffic look familiar.


## Development
Repo layout (simplified):
- `src/ui/` — tray, panel, status helpers, progress broadcaster
- `src/network/` — monitor, optimizer, speedtest runner, stealth
- `dist/` — HTML/CSS for the tray popover panel
- `tauri.conf.json` — window, tray, bundling

Useful scripts:
```bash
# Run the app (dev)
cargo tauri dev

# Lint / check
cargo check

# Bundle for distribution
cargo tauri build
```


## Contributing
Open issues, propose ideas, or drop a PR. Keep edits small, focused, and with a friendly description.


## License
MIT — do good things, don’t be shady.


