# HANDOFF (phases 4 and 5 + animation system + themes, palettes and notifications)

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
- **Frontend (`src`)**: Svelte 5 runes. `lib/scene.svelte.ts` (the UI owns the scene and pushes it throttled with `set_scene`), `lib/state.svelte.ts` (devices, toasts, confirm), `lib/profilesStore.svelte.ts` (store modules end in `Store` because `profiles.svelte` and `Profiles.svelte` collide on case-insensitive file systems: Windows/macOS), `lib/layout/` (physical keyboard table + layout providers, vitest), `LedCanvas.svelte` (SVG canvas + tools + history), `LayersPanel`/`EffectParams` (schema-driven), `DeviceView`, `AllDevices`, `Profiles`, `Settings`, `lib/i18n` (en/es JSON, `t()`, backend errors are `{code, detail}` translated as `err.<code>`).
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


## Themes, palettes and notifications (this round)

### Rust
- **Colours**: `themes/mod.rs` `Color = Rgb([u8;3]) | Slot{slot, lighten?}` (serde untagged, plain RGB files load unchanged). Slots: background, foreground, accent, primary, secondary, tertiary, alert, warn, ok, info, c1..c8. `Palette::new` derives missing slots; `generation` changes with the colours.
- **Resolution**: the stored scene keeps slots. `engine::resolve_scene` (scene layer params, any depth, including animation snapshots) runs in `Runner::scene_for`, cached by `(scene equality, palette generation)`: a steady scene costs nothing per frame. Overlays and animation resolution caches are keyed by palette generation too (`Geometry.palette`, `Tick.palette`).
- **Sources** (`themes/{omarchy,kde,pywal,gtk}.rs`, trait `Source` with `detect/read/watch_paths/poll_files`, injectable `Env` for fixtures in `themes/fixtures`). Omarchy reads `current/theme/colors.toml`, `theme.name`, and `theme/keyboard.rgb` only as accent fallback. `themes/ctl.rs` `ThemeCtl` (in `Engine.themes`) layers: source < profile overrides < game colours (`set_palette`), keeps the last good palette on errors, bumps an epoch that the runner turns into the `palette-changed` event. `themes/watch.rs`: `notify` on the source directories (debounce 300 ms) plus a 1 s fingerprint poll (Omarchy replaces whole directories on a switch). `themes/hook.rs`: the optional `theme-set` palette hook and the Omarchy event hooks (`55-chroma-desk-<type>`), installed with `omarchy hook install`, removed by exact file name.
- **Profiles** schema v2 (`theme`, `presets`, `triggers` = notifications, `enabled/builtin/requires`); v1 migrates by adding a Manual theme. `builtin.rs`: *Omarchy (follow theme)* (off by default, only listed when Omarchy is detected, enable state in `Settings.builtin_enabled`) and *Cyberpunk*. Their scenes are generated per connected device (`scene_for`); "Duplicate to edit" freezes it (`materialize`). New effect `glitch`; themed built-in animations (Accent breathing, Theme wave/chase, Flash, Pulse, Blink, Fill, Glitch, Neon Scan, Breach Alert; fixture `src/lib/mock/builtins.json` and `builtin_profiles.json` are generated by tests: `UPDATE_FIXTURES=1 cargo test fixture`).
- **Notifications** = `NamedTrigger` (animation, keys, colour `@slot|#hex`, duration/repeat/speed, devices, priority, replace, enabled, `sources{desktop, omarchy[]}`), per profile or global (`Settings.global_notifications`; the profile's wins). `triggers::start` resolves a name to a notification first, then to an animation; explicit CLI/socket fields win. `fire_desktop` / `fire_event` match rules. `desktop.rs`: zbus *monitor* connection on the session bus (opt-in `Settings.desktop_notifications`), reads only app name, urgency and summary; the test runs a private `dbus-daemon` with `busctl`.
- **CLI/socket**: `palette show|reload|clear|set|source`, `profile <name>`, `event omarchy:<type>`, `trigger <notification|animation> [--color @slot]`; socket cmds `set_palette`, `clear_palette`, `palette`, `palette_reload`, `palette_source`, `set_profile` (async hook in `socket::serve`), `event`.
- **Theme rules** ("when the Omarchy theme becomes X apply profile Y"): `ThemeCtl::set_name_watch` / `take_theme_change`, applied by the runner via `apply::apply_theme_rule` (emits `profile-applied`).

### Frontend
`palette.svelte.ts` (store + `resolveColor`, follows `palette-changed`), `ColorPicker.svelte` (Palette / Custom tabs, `@slot` chip; Escape closes without clearing a canvas selection), `Paint` is now `(Color|null)[]`; `ThemeEditor.svelte` (source, slots, overrides, pinned presets), Profiles cards (strip, built-in badge, Enable, Duplicate to edit), Settings -> Themes (palette, rules, hook command), Quick shows the profile's presets first, `Notifications.svelte` + `NotifEditor.svelte` + `notifications.svelte.ts` (templates, snippets, key picking on `LedCanvas`, sources). Toasts are click-through at the bottom centre (they used to cover buttons).
- Mock backend: `mock/themes.ts` simulates profiles, built-ins, palette events, Omarchy theme switches (`__chromaMock.fns.setOmarchyTheme`, `setPalette`, `fireDesktop`, `fireEvent`), hooks and notifications.
- e2e scenarios added: 27 (Advanced sizes on the Corsair and the X870), 100 (colour picker linking), 110 (built-in profiles, duplicate, theme editor, Omarchy follow), 120 (Settings themes, rules, hook), 130 (notifications). `t.setColor` / `t.pickSlot` drive the picker with real clicks.

### Open issues / notes
- KDE/pywal/GNOME are only verified with fixture files and "not available" on this machine (GNOME's `gsettings` does answer here).
- Desktop notifications: Wayland compositors that sandbox the bus or dbus-broker setups denying `BecomeMonitor` show `error: ...` as the listener status and retry every 30 s.
- A notification's `devices` use device keys (`name#serial`/`name@n`); a removed device is skipped while another chosen one still shows it.
- Built-in presets of effect kind still cannot be edited in the Draw editors (use Layers).


## System monitors, sequences and automations (this round)

### Rust
- `sysmon/` (`mod.rs`, `cpu.rs`, `gpu.rs`): `Metric` (`cpu.usage`, `cpu.temp`, `gpu.usage`, `gpu.vram`, `gpu.temp`, `ram.usage`; percent / degrees C). **Demand-driven**: `sysmon::reading/live/touch` register interest, a sampler thread reads twice a second for `TTL` (3 s) after the last request and then parks on a condvar, dropping NVML. Readers: `/proc/stat` deltas, hwmon (k10temp/zenpower/coretemp), NVML (`nvml-wrapper`, `nvidia-smi` fallback, at most one call per 2 s), AMD sysfs, `/proc/meminfo`. `Reading::smooth` glides between samples. Settings: `sysmon_gpu` (`auto` or `nvidia:0` / `amd:card2`). Commands `sysmon_status`, `sysmon_read`. Example `sysmon_demo`.
- Effects (`engine/effects.rs`): `meter` (path in pick order, `metric`/`value`, `min`/`max`, `sweep|live`, background `none` = transparent by default so the scene shows through and the last key blends by its fraction; `dim`/`off` paint a track), `blink` (exact count), `orbit` (laps, tail fades by alpha), `sequence` (steps with per-step effect/params/mask). Hidden from the Layers effect picker.
- `AnimKind::Sequence { steps: Vec<Step>, loop }` (`animations.rs`): `Step { effect, params, keys (ordered LedRefs), duration_ms, anim? }`; step effects `meter|blink|orbit|hold|animation|<any effect>`. Keys chosen but absent on a device = nothing shown (never "everything"). `cycle_secs`, `with_speed`, `recolor` support it.
- `profiles.rs`: `NamedTrigger.sequence` (inline sequence, wins over `anim`), `Sources` gained `ai`, `always`, `every{every_secs,for_secs}`, `threshold{metric,op,value,for_secs,hysteresis,cooldown_secs}`; all optional with serde defaults, so existing profiles/notifications load and re-serialise unchanged (test). The desktop `app` filter accepts a comma-separated list.
- `automations.rs`: the scheduler polled from the runner loop every 200 ms (a no-op without scheduled automations): *Always* keeps one hour-long overlay (looped sequence), *Every* starts D-second overlays, *Threshold* (`ThresholdState`: hold time, hysteresis re-arm, cooldown) touches the metric. A changed/removed/disabled automation restarts or stops its overlay.
- `triggers.rs`: `start_with(secs)`, `test_notification` (editor Test on unsaved automations, command `test_automation`), `fire_ai`/`clear_ai`; socket `{"cmd":"event","event":"ai:attention","from","kind"}` / `ai:clear`; CLI `chroma-desk event ai:attention [--from x] [--kind y]`.
- `templates.rs` (+ `src/lib/mock/templates.json`, `UPDATE_FIXTURES=1 cargo test templates_fixture`): gpu_monitor, cpu_monitor, discord, telegram, slack, matrix, ai_needs_you. Command `automation_templates`.
- `claude.rs`: merge/remove the Claude Code `Notification` hook in a settings.json *path* (backup `settings.json.chroma-desk-backup[-N]`, key order kept via serde_json `preserve_order`, invalid JSON untouched, idempotent). Commands `claude_status/install/remove` use `$CLAUDE_CONFIG_DIR` or `~/.claude`; tests only use temp files.
- Gotcha: `animations::cached` keys on the animation id, so tests rendering several variants of one animation need unique ids.

### Frontend
`Automations.svelte` (grouped cards, When chips, templates), `AutomationEditor.svelte`, `SequenceEditor.svelte` (steps with drag + arrows, ordered key picking on `LedCanvas` path tool, live preview), `WhenPicker.svelte`, `ClaudeSetup.svelte`, `sysmon.svelte.ts` (polling store, only while watched), `anim/live.ts` + TS renderer for meter/blink/orbit/sequence (parity tests in `anim.test.ts`). Settings: System monitor and AI assistants sections. Mock: `mock/sysmon.ts` (moving fake values, `fns.setMetric`), `mock/automations.ts` (scheduler port), fake Claude settings file. e2e `160-automations.mjs`.

### Open issues
- "Stop all" (button, `trigger --stop`, socket `stop`) now pauses Always automations (`Engine.paused_always`, kept while the automation is unchanged and enabled); Resume button, `trigger --resume`, socket `resume`, commands `resume_triggers` / `paused_automations`. Every/Threshold keep running (they are one-shot alerts).
- Audit round: Claude hook matching is "our command, optionally with extra flags" (a `--from codex` hook is never removed), settings.json symlinks are written through, scheduler durations cannot panic on hand-edited NaN/negative values. Live checks: `examples/monitor_demo.rs` (meters vs sensors) and `examples/desktop_demo.rs` (chat templates through a private dbus-daemon + stub notification daemon; compare only the template colour, the installed app also writes the keyboard).
- Intel GPUs report "not available" (no unprivileged usage counter).
