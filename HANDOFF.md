# HANDOFF (phases 4 and 5 + animation system)

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
- Vendored `openrgb2`: one doc comment added to satisfy `missing_docs`; keep docs on any further patches.
- The OpenRGB server of the user was found not running mid-session (it died when the dev instance was stopped); it was restarted as `openrgb --server --startminimized` (no GUI window) before the hardware tests.

## Animation system (blocks 1-5)
- **Rust**
  - `animations.rs`: `Animation` (kinds `effect` / `frames` / `path` / `palette`), portable `LedRef` (`name:Key: W` when the LED name is unique, else `idx:N`), resolution to the index-based effects `frames`/`path`/`palette` (in `engine/effects.rs`) cached per `(id, updated, device, LED count)`, JSON library in `~/.config/chroma-desk/animations` (atomic writes, corrupt files reported, unique names), 9 built-in presets shipped in the binary. `src/lib/mock/builtins.json` is a fixture kept in sync by a test (`UPDATE_FIXTURES=1 cargo test builtins_fixture`).
  - Layer `animation` = `{anim_id, snapshot}`: the profile carries a copy of the animation; the UI offers "Update" when the library entry is newer.
  - `engine/overlay.rs` + `triggers.rs` + `socket.rs`: overlays (priority, expiry, max 32, key masks), `chroma-desk trigger ...` CLI (talks to the running instance over the socket, never opens a window; deviates from the spec's single-instance forwarding because the socket is simpler and needs no Tauri runtime), 0600 Unix socket `$XDG_RUNTIME_DIR/chroma-desk.sock`, JSON lines <= 8 KiB, unknown fields rejected. Overlays only affect devices in Effects mode.
  - Commands: `anim_list/get/save/delete/duplicate/import/export`, `anim_apply` (backend only; the UI applies animations through the scene store), `trigger`, `stop_triggers`, `trigger_socket_path`.
  - `examples/anim_demo.rs` verifies frames/path/palette/overlay on the real keyboard by reading colours back (`engine`, `profile`, `readback`, `cleanup`). Run it with `XDG_CONFIG_HOME` pointing at a scratch dir.
- **Frontend**: Lighting tab levels Quick / Draw (Paint, Frames, Path, Palette) / Layers; `Animations.svelte` library; `anim/render.ts` (TS renderer used by thumbnails, editor preview and the mock engine), `anim/draft.ts` (editor model <-> portable animation), `editorSession.svelte.ts` (live preview via `previewHook` in `scene.svelte.ts`, which rewrites a copy of the scene on its way to the engine; unsaved-changes guard via `leaveGuard`/`navigate`), first-run `Tour.svelte`, Triggers section in Settings.
- **e2e**: `npm install --prefix e2e && npm run e2e` starts `MOCK=1 vite dev`, runs `e2e/scenarios/*.mjs` with real mouse/keyboard events in a fresh browser context each, writes screenshots to `e2e/screenshots/` (gitignored). The mock backend exposes `globalThis.__chromaMock` for assertions. HTML5 drag-and-drop of frames is tested with synthetic drag events only.

### Verified on the real Huntsman V3 Pro TKL (colours read back from the server)
Frames W -> A -> D looping, crossfade (W fades into A), path head advancing in stroke order, palette cells rotating in place, overlay via socket on WASD only and restored after expiry; real binary: socket mode 0600, `chroma-desk trigger` incl. errors/exit codes, README Python/socat examples; CPU ~1.1 % (debug build) with 2 overlays + a frames animation.

### Open issues
- Overlays ignore devices in hardware mode. Effect-kind presets cannot be edited in the Draw editors (use Layers).
- Thumbnails of effect presets are approximations of the Rust effects.
- A release `chroma-desk` started before this work may still be running with an old build; restart it to get the socket/triggers.
