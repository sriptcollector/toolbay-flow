import { useEffect, useRef, useState } from "react";
import { Action, ACTION_LABELS, api, on, Shortcut } from "../../api";
import { comboLabel, isMac } from "../../keys";
import { Keys, Page, Section, useSettings } from "../ui";

/** Records a key combo through Flow's native keyboard hook (works for Win/Fn and right-side modifiers). */
export function useRecorder(onDone: (keys: number[]) => void) {
  const [active, setActive] = useState(false);
  const [keys, setKeys] = useState<number[]>([]);
  const done = useRef(onDone);
  done.current = onDone;
  useEffect(() => {
    if (!active) return;
    let live = true;
    const off = on<{ keys: number[]; done: boolean }>("shortcut-recorded", (e) => {
      if (!live) return;
      setKeys(e.keys);
      if (e.done) {
        setActive(false);
        if (e.keys.length) done.current(e.keys);
      }
    });
    api.recordShortcut(true);
    return () => {
      live = false;
      off();
      api.recordShortcut(false);
    };
  }, [active]);
  return { active, keys, start: () => (setKeys([]), setActive(true)), stop: () => setActive(false) };
}

const WIN_MODS = [0x10, 0x11, 0x12, 0x5b, 0xa3, 0xa5];
const MAC_MODS = [63, 59, 58, 55, 56, 62, 61, 54];
const sig = (k: number[]) => [...k].sort((a, b) => a - b).join("+");
const generic = (k: number) => (k === 0xa3 ? 0x11 : k === 0xa5 ? 0x12 : k === 62 ? 59 : k === 61 ? 58 : k === 54 ? 55 : k);

/** Combos the OS keeps for itself; binding them would break the computer. */
const RESERVED_WIN = [
  [0x5b, 0x4c], // Win+L
  [0x5b, 0x44], // Win+D
  [0x5b, 0x45], // Win+E
  [0x5b, 0x52], // Win+R
  [0x5b, 0x09], // Win+Tab
  [0x12, 0x09], // Alt+Tab
  [0x12, 0x73], // Alt+F4
  [0x11, 0x1b], // Ctrl+Esc
  [0x10, 0x11, 0x1b], // Ctrl+Shift+Esc
  [0x11, 0x12, 0x2e], // Ctrl+Alt+Del
  [0x11, 0x43], // Ctrl+C
  [0x11, 0x56], // Ctrl+V
  [0x11, 0x58], // Ctrl+X
  [0x11, 0x5a], // Ctrl+Z
  [0x11, 0x41], // Ctrl+A
];

/** Returns a plain-language problem with a combo, or null if it's fine. */
export function checkCombo(action: Action, keys: number[], all: Shortcut[]): string | null {
  const mods = isMac ? MAC_MODS : WIN_MODS;
  const nonMods = keys.filter((k) => !mods.includes(k));
  const isF = (k: number) => (isMac ? [122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111].includes(k) : k >= 0x70 && k <= 0x87);
  if (!isMac && RESERVED_WIN.some((r) => sig(r) === sig(keys.map(generic)))) {
    return `${comboLabel(keys)} is used by Windows. Pick another combination.`;
  }
  if (nonMods.length === keys.length && keys.length === 1 && !isF(keys[0]) && !(action === "dismiss" && (keys[0] === 0x1b || keys[0] === 53))) {
    return `A single ${comboLabel(keys)} would fire every time you type it. Add a modifier like Ctrl or Alt, or use an F key.`;
  }
  if (nonMods.length > 1) return "Use at most one regular key plus modifiers.";
  const clash = all.find((s) => s.action !== action && sig(s.keys.map(generic)) === sig(keys.map(generic)));
  if (clash) return `${comboLabel(keys)} is already used for “${ACTION_LABELS[clash.action][0]}”.`;
  return null;
}

function ShortcutRow({ sc, all, defaults, onChange }: { sc: Shortcut; all: Shortcut[]; defaults: Shortcut[]; onChange: (keys: number[]) => void }) {
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const rec = useRecorder((keys) => {
    const problem = checkCombo(sc.action, keys, all);
    if (problem) {
      setError(problem);
      return;
    }
    setError(null);
    onChange(keys);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  });
  const [title, desc] = ACTION_LABELS[sc.action];
  const def = defaults.find((d) => d.action === sc.action);
  const isDefault = def && sig(def.keys) === sig(sc.keys);
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-title">{title}</div>
        <div className="row-desc">{desc}</div>
        {error && <div className="row-warn">{error}</div>}
      </div>
      <div className="row-ctl">
        <button
          className={`keybox ${rec.active ? "rec" : ""} ${saved ? "saved" : ""}`}
          title="Click, then press the new keys"
          onClick={() => (rec.active ? rec.stop() : (setError(null), rec.start()))}
        >
          {rec.active ? rec.keys.length ? <Keys keys={rec.keys} /> : <span className="hint">Press keys…</span> : <Keys keys={sc.keys} />}
        </button>
        {!isDefault && def && !rec.active && (
          <button className="link" onClick={() => (setError(null), onChange(def.keys))}>
            Reset
          </button>
        )}
      </div>
    </div>
  );
}

export default function Shortcuts() {
  const [s, save] = useSettings();
  const [defaults, setDefaults] = useState<Shortcut[]>([]);
  useEffect(() => {
    api.defaultShortcuts().then(setDefaults);
  }, []);
  if (!s) return null;
  const setKeys = (action: Action, keys: number[]) =>
    save({ shortcuts: s.shortcuts.map((x) => (x.action === action ? { ...x, keys: [...keys].sort((a, b) => a - b) } : x)) });
  const group = (names: Action[]) =>
    s.shortcuts
      .filter((x) => names.includes(x.action))
      .map((x) => <ShortcutRow key={x.action} sc={x} all={s.shortcuts} defaults={defaults} onChange={(k) => setKeys(x.action, k)} />);
  return (
    <Page
      title="Shortcuts"
      sub="Click a shortcut, then press the keys you want. Changes work right away, even while Toolbay Flow is hidden in the tray. Modifier-only combos like Ctrl + Win or Right Alt work too."
      actions={
        <button className="btn" onClick={() => save({ shortcuts: defaults })}>
          Reset all to defaults
        </button>
      }
    >
      <Section title="Dictation">{group(["push_to_talk", "hands_free", "command", "dismiss"])}</Section>
      <Section title="Text">{group(["paste_last", "copy_last", "transform1", "transform2", "transform3"])}</Section>
    </Page>
  );
}
