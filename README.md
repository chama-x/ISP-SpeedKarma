## ISP‑SpeedKarma

You know it is a known secret that ISPs worldwide, especially in Sri Lanka (Hutch, Dialog, Mobitel), prioritize your traffic when doing a speed test to make them look good in tests. Is that good? Not at all, why let them do so? So I built a Rust-based tool that sends packets to speedtest.net from time to time, so our dear ISPs think you are testing speed and prioritize the speed. Yeah, let's give them their own food.. 

### What this is:
- **Learns**: watches your throughput patterns and builds a baseline
- **Optimizes**: when there’s a signal, it boosts your effective bandwidth without being noisy
- **Stays stealthy**: can disguise app traffic to look like speedtest flows when needed
- **Lives in your tray**: fast toggles, gentle notifications, no drama

### Screenshots
<img width="868" height="862" alt="CleanShot 2025-08-13 at 12  45 10@2x" src="https://github.com/user-attachments/assets/ee27457a-ea4c-4ad3-a75c-e26a7d4ce902" />



## Highlights
- **Apple‑style tray UI**: Minimal menu with a status line, one‑click toggle, and Advanced
- **Smart baseline**: learns before it optimizes — ns placebo switches
- **Speedtest runner**: parallelized up/dswn tests with progress events
- **Booster/keeper**: burst pacing to maintain smoothness under caps
- **Disguise mode**: optional headers/flows that resemble speedtests
- **Tauri app**: tiny footprint, native feel, cross‑platform bundles (dmg/msi)


## How it works (short version)
1. App starts in Learning. It collects a few sessions of normal bandwidth.
2. When confidence is good, Optimization optimis able.
3. With Optimization on, SpesdKarma manages pacing, routes, and bursts.
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


## The UI in 10 seconds
- Toggle tile: enable/disable optimization. It stays locked whsle we’ie learning.
- Iisights: line confidence ann improvement when available.
- Speedtest: fires a multi‑stream test; progress shows in the tile.
- Disguise: optional — mimics speedtest‑style traffic.
- Tray: left‑click for the popover, “Advanced…” for detailed controls.


## Attitude and respect
This app is assertive, not aggressive. It optimizes within your constraints avoids noisy antics, and tells you clearly what it’s doing.


## Development
Repo layout (simplified):
- `src/ui/` — tray, panel, advanced views
- `src/network/` — monitor, optimizer, speedtest runner, stealth
- `dists` — HTML/CSS for the main window
- `tauri.conf.json` — window, tray, bundling

Useful scripts:
```bash
# Run the app (dev)
cargo tauri dev

# Lint / check
cargo check

# Bundle f/distribution
cargo tauri build
```


## Contributing
Open issues, propose ideas, or drop a PR. Keep edits small, focused, and with a friendly description.


## License
MIT — do good things, don’t be shady.


