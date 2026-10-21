# Parity evidence: bar menu, sounds, transforms, auto-learn toast

Checked 2026-10-21 against Wispr Flow 1.6.75 for Windows (bundle extracted to
`C:\Users\orion\rea-work\wispr-675\.webpack`). This file records behaviour only, in our own words.
Offsets are byte positions in the named file. Method as in `dictation-core.md`: REA application
analysis of the main bundle, then an inert static read of the large files, then our own tests
(`cargo test`).

## Flow bar right-click menu (renderer/contextMenu/index.js @~146900)
- Visible items in order: Hide for 1 hour, Settings, divider, Microphone (submenu, the mic in use
  marked), divider, Transcript history, Paste last transcript. Reminder and formatting items are
  hidden by default.
- Hide for 1 hour snoozes only the resting pill. A notice says it's hidden for an hour, with Undo
  (6 s; main index.js, BarHidden notification). The original shows that notice at most twice.
- **Ours now:** same items and order ("Dictation history" opens Home). Hiding keeps the bar window,
  so the notice and any dictation still show, and only the idle pill hides. Undo clears the snooze.
- **Different:** we show the notice every time, not only twice.

## Sounds (renderer/hub/index.js @~508800, main index.js @4103761)
- Slots in every sound pack: start, stop, hands-free lock, paste, achievement, and three notification
  sounds (success, error, alert). The paste sound is behind a flag that's off by default.
- Start and stop are triggered by the native helper on the hotkey itself. Error notifications play
  the error sound. The "cancelled" notice is silent, and there is no cancel sound.
- **Ours now:** start, stop, lock, error. We removed our cancel sound.
- **Different:** our start sound plays once the mic opens, a few ms after the key. We don't play
  the alert sound with the 19-minute warning, and we have no achievement sound (no milestones).

## Transforms (main index.js: handler @~4801000, runner near the PolishDisabledDuringDictation check)
- Each transform shortcut fires on key-down. Slot 1 falls back to the Prompt Engineer built-in when
  nothing is saved there, and the other slots do nothing when empty. Built-ins seen: Polish, Prompt
  Engineer, plus "turn to list" and "translate to English" suggestions.
- The selected text comes from accessibility, or a copy when that's empty. It needs 1 to 1000 words.
  The request gives up after 60 s.
- Messages: select text first (with the shortcut, longer the first time), too long (under 1000
  words), "looks good" when nothing changed (3 s), empty result, timed out, "<name> cancelled", and
  can't paste (text left on the clipboard).
- It won't run while dictating ("doesn't work while dictating"), and dictating while a transform
  runs is refused ("can't dictate while <name> is running").
- **Ours now:** all of the above except the can't-paste case. Ours keeps a 150 ms beat before
  reading the selection. Tests: `decide()` covers the idle-only and refused-while-dictating rules.
- **Different:** our three defaults are Polish, Prompt engineer and Make it shorter, on
  Win+Alt+1/2/3. The original defines Win+Alt+1/2/3 but ships no transform shortcut in its default
  map. There's no can't-paste fallback yet (see failed-paste detection in `dictation-core.md`).

## Auto-learn toast (main index.js, LearnedDictionaryWords)
- "Added <terms> to your Dictionary", where the terms are curly-quoted and joined as a list
  ("a", "b", and "c"). Undo removes them. 6 s, silent.
- **Ours now:** same format (`quoted_list`, tested), 6 s, Undo.
- **Not compared:** how the original notices edits. Its native helper reports the edited text,
  and that code isn't in the JavaScript bundle. Ours polls the field every 2.5 s for 30 s.
