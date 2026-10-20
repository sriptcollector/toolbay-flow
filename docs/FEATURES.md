# Feature checklist (v0.1.0)

Checked against the Wispr Flow reverse-engineering report (toolbay-research, sections on product, pipeline and UI).
"Verified" means exercised on Windows 11 + RTX 4070 Ti with the installed release build.

| Report feature | Toolbay Flow | State |
|---|---|---|
| Push-to-talk hotkey (hold Ctrl+Win / Fn) | Low-level keyboard hook, modifier-only chords | Verified (live dictations pasted into Chrome and the Flow window) |
| Hands-free (dedicated chord + double-tap PTT) | Ctrl+Win+Space or double-tap; stop/cancel on the bar | Built, unit-tested state machine |
| Esc to dismiss | Swallowed only while listening | Built, unit-tested |
| Start-menu suppression for Win chords | Dummy-key masking | Built |
| Floating pill / Flow bar | Rest sliver, hover, waveform, hands-free buttons, processing, done, error, toasts; follows cursor monitor; click-through outside the pill | Built |
| Mic capture, device picker | cpal, any input device, 16 kHz resample, level meter, mic test | Verified (mic test in onboarding) |
| Transcription | whisper.cpp server, model downloader Tiny → Large v3 Turbo, CUDA on NVIDIA | Verified: ~0.1 s per 10 s clip on GPU |
| AI cleanup: fillers, punctuation, lists, backtracking, spoken commands, email layout | Local Qwen3 via llama.cpp (Vulkan/CPU) + rule fallback; 4 levels | Verified: ~0.2 s; "6, no wait, 7" → "7"; lists; email layout |
| Per-app styles (personal/work/email/other × formal/casual/very casual/excited) | Same categories + custom app rules | Verified (category routing in selftest) |
| Personal dictionary | Words, replacement rules, star, usage, speech-model biasing | Built |
| Auto-learn from corrections + undo toast | Watches the field after paste via UI Automation | Built (not yet exercised live) |
| Snippets | Trigger phrase → text, whole-dictation and inline | Built, unit-tested |
| Command mode (edit selected text by voice) | Ctrl+Win+Alt; selection via UIA or Ctrl+C fallback | Built (not yet exercised live) |
| Transforms / Polish (Win+Alt+1/2/3) | 3 editable presets | Built (not yet exercised live) |
| Context: focused app, window title, text around cursor | Windows UI Automation; macOS AX code paths | Verified on Windows (app names recorded) |
| Paste into any app with clipboard restore | Clipboard snapshot of all formats, Ctrl/⌘+V, restore, excluded from clipboard history | Verified (Chrome) |
| Paste last / copy last | Alt+Shift+Z / X, tray item | Built |
| History (copy, retry, revert view, delete, search) | Yes, with 14-day audio retention option | Built |
| Data policies (keep / 24 h / never) | Yes | Built |
| Languages (99, auto-detect) | Yes, with multilingual models | Built |
| Onboarding (privacy, permissions, mic test, shortcut test, language, try it) | Yes + hardware-aware model download | Verified (completed by Orion on this PC) |
| Settings (launch at login, sounds, mute music, bar, theme…) | Yes; synthesized sounds, no audio files | Built |
| Stats / insights (WPM, streaks, per-app, heatmap, time saved) | Yes | Built |
| Scratchpad notes | Plain-text notes | Built (no rich text, no sync) |
| Tray menu | Open, paste last, mic, language, bar toggle, shortcuts, quit | Built |
| Plans, paywalls, sign-in, word limits | None by design | n/a |

## Not included / remaining

- Meeting notetaker (recording, speakers, summaries, calendar), cloud sync, teams/admin, referral, mobile apps.
- OCR of screenshots for context (Wispr sends screenshots to its server; Flow only reads accessibility text).
- "Hey Flow"/agent-style Instruct mode with tool calls; MCP server.
- Rich-text (HTML) snippets and Scratchpad formatting.
- Browser URL detection (window title is used instead).
- GPU speech on AMD/Intel GPUs on Windows (CPU today; needs a Vulkan build of whisper.cpp).
- macOS app is built by CI but untested on a Mac (needs Accessibility + Input Monitoring + Microphone grants).
- Code signing (Windows SmartScreen will warn on first run; macOS DMG is unsigned/unnotarized).
