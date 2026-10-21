# Parity evidence: dictation core

Checked 2026-10-21 against Wispr Flow 1.6.75 for Windows (main-process bundle extracted to
`C:\Users\orion\rea-work\wispr-675\.webpack\main`, sha256 `f618e744…d39f82`). This file records
behaviour only, in our own words. No code, wording or assets were copied.

## How it was checked

- REA `analyze_javascript_application` on the extracted main bundle (evidence
  `ev_2e10cafe…28e74`, summary view `ev_4444a6ba…cb12a`). The semantic pass ran out of heap on
  `index.js`, so the dictation module was read as inert text by byte offset (offsets below).
  Nothing was executed and no Wispr server was contacted.
- REA `evaluate_reconstruction_coverage`, boundary `flow.dictation-core`: 7 surfaces, each with an
  owner file in this repo. Result **partial**: original side observed, our side verified by
  `cargo test` (REA can't capture processes on Windows yet, so the verifier runs outside REA).
  Inputs: `C:\Users\orion\rea-work\flow-parity\coverage.json`.
- Our side: `cargo test` (the `decide()` tests in `dictation.rs`, the hotkey matcher tests, the
  press-enter and context tests). Not yet repeated live on Orion's keyboard. That needs his OK.

## What the original does (and what we now do)

Offsets are byte positions in `index.js`.

### Timing constants (@3913812, @4005687, @3930835)
- Tap threshold: 500 ms. Recording warning at 19 min, forced stop at 20 min, then a notice.
- A recording that has received no audio at all after 5 s is dropped with a "no audio" error.

### Push-to-talk (@3991806, other-key drop @4799803)
- Press while idle starts listening.
- Release ≥ 500 ms after the press: stop and transcribe.
- Release sooner: it's a tap. The mic **stays open** for another 500 ms. If push-to-talk is pressed
  again within 500 ms of the *first* press, the same recording locks into hands-free (lock sound).
  If not, the recording is dropped silently.
- While locked, a press within 500 ms of the start (a third quick tap) cancels. A later press
  stops and transcribes. Releasing does nothing.
- Pressing an unrelated key within 1 s of starting push-to-talk (the user was reaching for another
  Ctrl+Win shortcut) drops the recording.
- A press while the last dictation is still processing shows a "still working" notice with a
  Cancel button (6 s), unless it came within 500 ms of the last action.

**Before this change** we used 300 ms for a tap and a fresh hands-free session 450 ms after
release (the first tap's audio was thrown away), had no triple-tap cancel, no other-key drop,
no busy notice, no 19-minute warning and no no-audio guard.

### Hands-free chord (@3997350)
- Idle: start locked. Locked and within 500 ms of the start: cancel (quick double press).
  Locked later: stop. Holding push-to-talk or in command mode: switch to locked dictation.

### Command mode (@3992783)
- If it's turned off: a notice pointing to Settings, at most 3 times and once a day.
- Idle: start in command mode. While push-to-talk is held: switch to command mode. While locked:
  no switch. Release stops at once (no tap window).

### Cancel (Esc, bar cancel button)
- Cancels while starting, listening, stopping or processing.
- Shows a short "cancelled" notice (3 s) with an action that brings the dictation back.
  Cancelled audio is kept, and paste-last on a cancelled or failed dictation re-runs that audio
  instead of pasting older text. Quick-tap and triple-tap cancels show no notice.

### Paste last / copy last (@4800138)
- Paste last fires once **every key of the shortcut is released**, then 25 ms later, so the
  user's Shift/Alt aren't still held when the app presses Ctrl+V. We used to paste 120 ms after
  the key went *down*, which could send Ctrl+Shift+Alt+V.
- Copy last puts plain text on the clipboard and confirms on the bar.

### Spoken "press enter" (@3894834)
- Only the words "press enter", only at the very end of a dictation (trailing punctuation
  allowed), and not in command mode. A sentence's own end mark before it is kept. If nothing else
  was said, Enter is pressed without pasting. Enter is sent right after a successful paste.
- We used to accept "hit enter"/"press return" too. Now it's "press enter" only.

### Text around the cursor
- Up to 500 characters on each side of the cursor, cut back to a whole word. Nothing is read from
  the app's own windows. We used 600 before and 300 after.

### Tray (@5411540)
- Order: open, paste last with a greyed preview of that text, shortcuts, microphone, languages,
  then help/feedback and quit. Ours now follows that order. Settings and the bar toggle are extra
  items with no counterpart (the original has no settings item in the tray).

## Still different (open)

- **Failed-paste detection.** The original's helper puts text on the clipboard with delayed
  rendering, notices when no app reads it, and shows a "paste failed" notice. We restore the
  clipboard after 700 ms and can't tell whether the paste landed.
- **First "press enter".** The original doesn't press Enter the first time. It shows a one-time
  tip with enable/disable. Ours is on by default with no tip.
- **Command mode tips.** The original shows a one-time "using command mode" tip (with disable)
  and a short label each time. Ours only colours the bar.
- **Long-hold tip.** After a push-to-talk hold of 1–10 min the original suggests hands-free (max 3
  times, hourly). Not built.
- **Warning sound.** The original's 19-minute warning plays a sound. Ours is silent.
- **Cleanup, "new line", "scratch that", punctuation words.** These run on Wispr's servers, so
  the shipped app can't show what they do. They can only be compared by dictating the same audio
  into both apps, which needs Orion's OK to drive his keyboard.
- **Live run.** None of the above was repeated live yet, on either app.
