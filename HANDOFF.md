# HANDOFF (phases 4 and 5)

## Architecture
- **Rust (`src-tauri/src`)**
  - `sdk.rs`: OpenRGB SDK helpers (DTOs, set_leds, modes). `state.rs`: `AppState` (Arc-shared: connection, config, engine, tray, ready flag), zone-size restore/migration.
  - `engine/effects.rs`: 11 pure effects, schema (`effect_defs`) is the single source of defaults/UI. Randomness is a hash of `(seed, led, time slot)`, so effects are stateless (deviation from the plan's `&mut rng` signature; same determinism).
  - `engine/compositor.rs`: layers bottom to top, masks (all/zone/LEDs), opacity, blend normal/add/multiply/screen.
  - `engine/mod.rs`: `Scene` model (serde), fps defaults/caps, `is_due`, global-sync slots, `render_device`.
  - `engine/runner.rs`: 60 Hz tokio task; per-device fps cap; frame diffing; 10 s keep-alive resend; `frame` events (<=30 fps, visible devices only); after 3 consecutive failures drops the connection and reconnects with backoff (1..10 s, never spawns the server); events `engine-status`, `devices-changed`.
  - `profiles.rs` (JSON files, schema_version + migration, atomic writes via `fsutil.rs`), `config.rs` (settings, zone sizes, legacy migration), `identity.rs` (device keys `name#serial` / `name@ordinal`).
  - `apply.rs` (profile apply, startup sequence, blackout), `tray.rs`, `cli.rs`, `server.rs` (OpenRGB window detection, systemd user unit), `commands.rs` (Tauri commands).
  - `examples/engine_demo.rs` (drives the real hardware, prints fps/CPU and reads the keyboard back), `examples/profile_demo.rs`, `examples/probe.rs`.
- **Frontend (`src`)**: Svelte 5 runes. `lib/scene.svelte.ts` (the UI owns the scene and pushes it throttled with `set_scene`), `lib/state.svelte.ts` (devices, toasts, confirm), `lib/profiles.svelte.ts`, `lib/layout/` (physical keyboard table + layout providers, vitest), `LedCanvas.svelte` (SVG canvas + tools + history), `LayersPanel`/`EffectParams` (schema-driven), `DeviceView`, `AllDevices`, `Profiles`, `Settings`, `lib/i18n` (en/es JSON, `t()`, backend errors are `{code, detail}` translated as `err.<code>`).
- `MOCK=1 npm run dev` serves the UI against `src/lib/mock/tauri.ts` (browser only). Dev aid: URL hash `#d=<index>` / `#profiles` selects a view; headless `chromium --screenshot` works for visual checks.

## Run / test
```
cd src-tauri && mise exec -- cargo test && mise exec -- cargo clippy --all-targets -- -D warnings
npm run check && npm test
mise exec -- cargo run --release --example engine_demo 8     # live hardware (SYNC=1 for the global scene)
npm run tauri dev                                            # app
npm run tauri build -- --no-bundle                           # release binary (verified)
```

## Verified on real hardware
- Engine on all 11 devices: rainbow + WASD red layer on the Huntsman (read back from the server: W/A/S/D = 255,0,0, Q/E rainbow), chase on the Corsair, global sync run; CPU ~0.3 % of a core.
- Legacy-less config -> zone-size keys by identity, profile save/apply/list/delete against the live server (`profile_demo`).
- App binary: `--hidden` (no window, tray created), single instance (second launch focuses the window), close-to-tray (window closed, process alive), `--apply-profile` with a hardware-only profile exits right after applying.

## Known limitations / not done
- Not interactively tested in the real window (screenshots of the real webview are not possible here): LedCanvas painting with the mouse, drag-reorder, native file dialogs (import/export), tray menu clicks, autostart toggle, systemd service toggle were only compile/unit checked or checked in the mock UI. The autostart entry was NOT enabled; the systemd unit was NOT installed/enabled.
- XDG autostart on Hyprland/uwsm: `systemd-xdg-autostart-generator` is present and other `~/.config/autostart` entries run through it, but a chroma-desk entry was not exercised.
- OpenRGB "GUI open" detection is a heuristic (an `openrgb` process started without `--startminimized`). If the user's server is the GUI process itself the warning will show.
- Lights off: hardware-mode devices without a recorded native mode stay black after "Lights on" until touched.
- Tray availability cannot be detected when no StatusNotifier host exists (icon build succeeds); in that case closing hides the window with no way back except relaunching (single instance then shows it).
- Quit from the tray asks the UI (event `request-quit`) to check unsaved changes; closing the window does not warn (it hides).
- Removing the old `rgb-sync` (plan 5.3 last bullet) was NOT done: needs the user's consent.
- Zero-LED ARGB header zones are saved as size 0 in config/profiles.
- Fire effect (optional) not implemented. Light theme variables exist but there is no theme switch.
- `PKGBUILD` was only syntax-checked (`bash -n`); it builds from the checkout (`packaging/arch`). The release build itself was verified.
- Vendored `openrgb2` still emits a `missing_docs` warning count of 0 after our doc fix, but any future patches must keep docs.
- The OpenRGB server of the user was found not running mid-session (it died when the dev instance was stopped); it was restarted as `openrgb --server --startminimized` (no GUI window) before the hardware tests.
