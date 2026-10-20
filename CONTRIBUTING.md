# Contributing to Toolbay Flow

Thanks for helping. Read [AGENTS.md](AGENTS.md) first: it's the rulebook for people and coding agents alike.

The short version:

- **Simplicity first.** Small, clear changes. Good defaults over settings.
- **One-step install that includes the models.** Install once and dictation works, offline.
- **1:1 feature and UX parity with Wispr Flow is the bar.** Check [docs/FEATURES.md](docs/FEATURES.md).
- **Consumer-first.** No terminals, no technical prompts, plain-language errors, works on an 8 GB laptop without a GPU.
- **Everything free.** No paywalls, limits or required accounts.
- **Local by default, no telemetry.**
- **Clean-room only.** Your own code, wording, icons and sounds. Never copy the original app.

Before opening a pull request: `cargo test --manifest-path src-tauri/Cargo.toml`, `npx tsc --noEmit`, and try a dictation.
