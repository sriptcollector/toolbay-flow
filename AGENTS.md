# Toolbay Flow: rules for contributors and coding agents

Toolbay Flow is a free, open-source rebuild of Wispr Flow. It's part of Toolbay, where every
tool is a free version of a paid app and opens from the Toolbay hub. These rules apply to every
change, and the "Toolbay tool rules" apply to every Toolbay app.

## Toolbay tool rules (every Toolbay app)

1. **Consumer-first.** Built for people who never open a terminal. Double-click the installer and
   it works: no Python, CUDA, drivers or model choices, no admin prompt (per-user install).
   Onboarding is plain language and walks through permissions. It runs on an average 8 GB laptop
   with no GPU. It updates itself, recovers from crashes, and every error says what happened and
   what to do, in plain words.
2. **Simplicity first.** The smallest design that works. No setting where a good default will do.
3. **One-step install, models included.** Whatever the app needs to work (AI models included)
   comes with the one install. Flow bundles a starter speech model and engine in the installer
   and upgrades to better models quietly in the background.
4. **Automatic model choice.** The app picks models for the user's hardware (GPU vendor and memory,
   RAM, CPU cores) so it stays instant, measures real latency and steps down if it's slow, and
   re-checks when hardware changes. Manual choice exists only under "Advanced".
5. **1:1 parity with the original is the bar.** Every screen, setting, hotkey, voice command,
   state and flow of the original app, in the same order and with the same behaviour. A parity
   checklist lives in `docs/FEATURES.md`; nothing ships as "done" while items are missing.
6. **Everything free.** No plans, paywalls, word limits, trials or required sign-in.
7. **Local by default, no telemetry.** Nothing leaves the computer unless the user turns on
   something that obviously needs the network (for example their own cloud API key). No
   analytics, no crash uploads, no phoning home.
8. **Clean-room only.** Rebuild behaviour from observation and public information. Never copy the
   original's code, binaries, sounds, fonts, icons, screenshots or wording. Write your own.
9. **Branding.** The app has its own original logo and icon. It may echo the original's visual
   family (shape language, weight, colour mood) so people recognise the category, but it must be
   a different drawing that can't be mistaken for the original's mark, shown with the Toolbay name.
   - The credit line is "Inspired by <original>" with the original's official logo, small and
     unmodified, plus "Independent open-source project, not affiliated with or endorsed by
     <company>." right beside it.
   - The original's logo appears only inside that credit: onboarding/about, the settings footer,
     the README header and the repo social preview. Nowhere else.
   - Both come from one config entry (`brand.json` → `inspiredBy`: name, company, logoUrl,
     website, notAffiliated), so the credit can be removed everywhere in one change.
10. **Theme.** Use the shared Toolbay theme tokens (`src/theme/toolbay-theme.css`, the same file
    the hub uses) with the original app's layout and feel.
11. **Lives under the hub.** Ship a `toolbay.json` install manifest with each GitHub release.
    Support `--from-toolbay`, `--show`, `--hide`, `--quit`, single-instance, a `running.json` ({pid, visible, window, updatedAt}, rewritten on every show/hide)
    state file, and the hub's `TOOLBAY_TOKEN` (memory only, never written to disk).

## Working on Flow

- `npm run tauri dev` to run, `npm run tauri build` for the installer (it fetches the bundled
  engine and starter model via `scripts/fetch-bundled.mjs`).
- `cargo test --manifest-path src-tauri/Cargo.toml` must pass.
- `toolbay-flow --selftest <whisper-url> <llama-url> <wav>@<app>…` runs recordings through speech
  and cleanup headlessly; `--bench <whisper-url> <llama-url> <runs> <wav>…` measures
  release-to-text latency. Target: under 500 ms p50 for short dictation on a GPU machine.
- Never mute, duck or pause other apps' audio.
- Release: bump `version` in `package.json` and `src-tauri/tauri.conf.json`, tag `vX.Y.Z`; CI builds
  Windows + macOS, publishes the release and attaches `toolbay.json` with sizes and checksums.
