# Parity checklist vs Wispr Flow (v0.4.0)

Source: the reverse-engineering report in toolbay-research (evidence/03 product/UI, 01 architecture,
02 pipeline). "Verified" = exercised on Windows 11 + RTX 4070 Ti. "Built" = implemented, unit-tested
or rendered, not yet exercised live by a person. "Omitted" = deliberately left out (paid, cloud or
team-only features that don't apply to a free local app).

## Dictation core

| Feature | State |
|---|---|
| Hold-to-talk (Ctrl+Win / Fn): 500 ms tap window, drop on another shortcut within 1 s, 19-min warning, 20-min stop, no-audio guard | Verified live: Notepad, Chrome, Claude, the Toolbay hub and Flow's own windows. Parity: matched (traced in the original's bundle, `decide()` tests; docs/parity/dictation-core.md) |
| Hands-free: Ctrl+Win+Space, double-tap PTT (same recording), triple tap or quick double chord cancels, stop/cancel buttons on the bar | Built, state machine unit-tested. Parity: matched (traced, `decide()` tests; docs/parity/dictation-core.md) |
| Command mode (Ctrl+Win+Alt): edit selection by voice, or write from an instruction; "turned off" hint | Built. Parity: matched (hotkey states and hint traced, `decide()` tests; the model's output is not compared; docs/parity/dictation-core.md) |
| Esc cancels while listening or processing (only then), "cancelled" notice with Undo that brings the dictation back | Built, unit-tested. Parity: matched (traced, `decide()` tests; docs/parity/dictation-core.md) |
| Paste last (Alt+Shift+Z, fires once its keys are up; re-runs a cancelled or failed dictation) / copy last (Alt+Shift+X) | Built, matcher-tested. Parity: matched (traced, hotkey and `decide()` tests; docs/parity/dictation-core.md) |
| Transforms 1-3 (Win+Alt+1/2/3): Polish, Prompt engineer, Make it shorter, editable; 1-1000 words, 60 s limit, messages for no selection / no change / empty / too long / cancelled, refused while dictating | Built, `decide()` tests. Parity: matched (traced; the third built-in differs, see docs/parity/bar-sounds-transforms.md) |
| Shortcut editor: live recorder, modifier-only and Right Ctrl/Alt, conflicts, reserved combos, reset, instant apply | Built, regression-tested (the "can't change hotkeys" bug) |
| Start-menu suppression for Win chords | Built |
| Speech: local whisper.cpp, GPU (CUDA) when present | Verified: ~0.1 s for 10 s of audio |
| Automatic model choice from hardware, latency step-down, manual override in Advanced | Built (auto choice verified on this PC) |
| One-step install with a bundled starter model and engine | Built (installer verified) |
| Streaming transcription while speaking, pre-roll, warm mic, silence compaction | Built; benchmark below |
| AI cleanup: fillers, self-corrections, punctuation, lists, numbers, email layout | Verified on recordings |
| Spoken "press enter" at the end of a dictation (Enter after paste, Enter alone if nothing else was said) | Built, unit-tested. Parity: matched (rule traced in the original's bundle; `textproc` test; docs/parity/dictation-core.md) |
| Spoken commands: new line, new paragraph, punctuation words, "scratch that" | Built, unit-tested (the original does these on its servers; compare by dictating into both apps) |
| Per-app style categories × Formal / Casual / Very casual / Excited, custom app rules | Verified routing |
| Context: focused app, title, 500 characters each side of the cursor (cut on a word), selection; never password fields | Verified on Windows (text window aligned with the original; password handling not yet traced) |
| Paste via clipboard + Ctrl/⌘+V, all-format clipboard restore, kept out of clipboard history | Verified |
| Never mutes or ducks other audio | Built (mute feature removed; shared-mode console capture) |
| No dropped words: adaptive silence detection, safe streaming cuts, mic pre-open on first hotkey key, 250 ms tail | Verified: a real 23 s dictation went from 51% to 97% word recall; regression clips pass (eval/pipeline.mjs) |
| Engine: local (default) or cloud with your own key (Groq/OpenAI-compatible), local fallback | Built (cloud not tested without a key) |
| Debug recordings (audio + raw + final text, last 20) | Built |
| Personal dictionary: words, replacement rules, star, usage counts, speech-model biasing | Built |
| Auto-learn corrected words + "Added X" toast with Undo | Built, unit-tested. Parity: matched (toast wording style, list format, 6 s, Undo traced; how edits are watched is native code in the original and not compared; docs/parity/bar-sounds-transforms.md) |
| Snippets with length counter | Built |
| Sync across computers with a Toolbay account: settings, styles, dictionary, snippets, transforms, shortcuts (per OS), engine choice, onboarding; optional last-50 history; never audio or API keys | Built; two-computer tests + live round trip against toolbay.ai |
| 99 languages / auto-detect | Built |

Parity coverage (`node scripts/parity.mjs`): share of rows checked against the real app with the REA tools,
marked "Parity: matched". Every release note states it.

Release-to-text latency on this PC (p50 / p95, 20 runs, includes AI cleanup):
3 s speech 159 / 185 ms · 10 s 303 / 332 ms · 30 s 845 / 893 ms.

## Screens

| Original screen / element | Toolbay Flow |
|---|---|
| Look: warm-paper light theme + dark, frameless window with own title bar, 216px collapsible sidebar, rounded content panel, serif display headings, banners, underlined tabs | Built; checked side by side against the original's media-kit screenshots (Dictation, Insights, Dictionary, Snippets, Style). Parity: matched (screens) |
| Onboarding: welcome | Built, with "Inspired by" credit |
| About you (role, where you heard) | Built (stays local) |
| Where you spend time typing | Built |
| Privacy | Built |
| Permissions (mic; macOS accessibility + input monitoring) with guidance | Built (Windows mic check verified to run; macOS untested) |
| Mic test with bars + "do you work next to people?" | Built |
| Language | Built |
| Shortcut test ("do the keys light up?") | Built |
| Try it yourself: message, email, doc practice screens | Built (own generic mock-ups, no third-party brands) |
| Personalize styles | Built |
| Speedy onboarding, trial offers, referrals, team invites, Flow Pro | Omitted (free; the optional Toolbay account only syncs settings) |
| Home: stats strip + searchable history by day (copy, original transcript, retry, delete) | Built |
| Insights: words, WPM, streak, time saved, heatmap, per-app | Built |
| Dictionary, Snippets, Style, Transforms pages | Built |
| Scratchpad notes | Built (page in the hub; no floating window, no rich text) |
| Settings: general, voice, cleanup, experimental, data & privacy, about | Built |
| Flow bar: rest sliver, hover, waveform, hands-free buttons, command colour, processing, done, error, toasts, follows monitor, click-through | Built; sizes, colours, waveform and timing from the original's shipped CSS. Parity: matched (static) |
| Flow bar right-click menu, in the original's order: hide for 1 hour (with Undo), settings, microphone, dictation history, paste last | Built. Parity: matched (traced; docs/parity/bar-sounds-transforms.md) |
| Tray menu, in the original's order: open, paste last + preview of the text, shortcuts, microphone, language, then settings, bar toggle, share feedback, quit | Built. Parity: matched (order traced; Settings and the bar toggle are our extras; docs/parity/dictation-core.md) |
| Share feedback (feedback, bug, idea, show off) to the Toolbay Flow community on toolbay.ai; optional private debug info for bugs | Built |
| Sounds: start, stop, hands-free lock, error; cancelling is silent | Built (own synthesized sounds). Parity: matched (sound set and when each plays traced; docs/parity/bar-sounds-transforms.md) |

## Not built yet

- Meeting notetaker (recording, speakers, summaries, calendar, imports).
- Floating scratchpad window with rich text; rich-text (HTML) snippets.
- Screenshot OCR for context; browser URL detection (window title is used).
- "Hey Flow" / Instruct agent mode with tool calls; MCP server.
- GPU speech on AMD/Intel GPUs on Windows (CPU today; cleanup already uses Vulkan).
- Bar docking to the left/right screen edge.
- macOS: built in CI, not tested on a real Mac.
- Code signing: prepared, needs certificates (docs/SIGNING.md).
