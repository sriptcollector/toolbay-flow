import { useEffect, useState } from "react";
import { Action, ACTION_LABELS, api, on, Shortcut } from "../../api";
import { Keys, Page, Section, useSettings } from "../ui";

/** Records a key combo through the native hook (works for Win/Fn chords). */
export function useRecorder(onDone: (keys: number[]) => void) {
  const [active, setActive] = useState(false);
  const [keys, setKeys] = useState<number[]>([]);
  useEffect(() => {
    if (!active) return;
    api.recordShortcut(true);
    const off = on<{ keys: number[]; done: boolean }>("shortcut-recorded", (e) => {
      setKeys(e.keys);
      if (e.done) {
        setActive(false);
        if (e.keys.length) onDone(e.keys);
      }
    });
    return () => {
      off();
      api.recordShortcut(false);
    };
  }, [active]);
  return { active, keys, start: () => (setKeys([]), setActive(true)), stop: () => setActive(false) };
}

function ShortcutRow({ sc, onChange, conflict }: { sc: Shortcut; onChange: (keys: number[]) => void; conflict: boolean }) {
  const rec = useRecorder(onChange);
  const [title, desc] = ACTION_LABELS[sc.action];
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-title">{title}</div>
        <div className="row-desc">{desc}</div>
        {conflict && <div className="row-warn">Same keys as another action</div>}
      </div>
      <div className="row-ctl">
        {rec.active ? <span className="recording">{rec.keys.length ? <Keys keys={rec.keys} /> : "Press the keys…"}</span> : <Keys keys={sc.keys} />}
        <button className="btn small" onClick={() => (rec.active ? rec.stop() : rec.start())}>
          {rec.active ? "Cancel" : "Change"}
        </button>
      </div>
    </div>
  );
}

export default function Shortcuts() {
  const [s, save] = useSettings();
  if (!s) return null;
  const setKeys = (action: Action, keys: number[]) =>
    save({ shortcuts: s.shortcuts.map((x) => (x.action === action ? { ...x, keys: [...keys].sort((a, b) => a - b) } : x)) });
  const sig = (k: number[]) => [...k].sort().join("+");
  return (
    <Page
      title="Shortcuts"
      sub="These work everywhere, even when Flow is in the background. Modifier-only combos like Ctrl + Win are fine."
      actions={
        <button className="btn" onClick={() => api.defaultShortcuts().then((d) => save({ shortcuts: d }))}>
          Reset to defaults
        </button>
      }
    >
      <Section title="Dictation">
        {s.shortcuts
          .filter((x) => ["push_to_talk", "hands_free", "command", "dismiss"].includes(x.action))
          .map((x) => (
            <ShortcutRow key={x.action} sc={x} conflict={s.shortcuts.some((y) => y !== x && sig(y.keys) === sig(x.keys))} onChange={(k) => setKeys(x.action, k)} />
          ))}
      </Section>
      <Section title="Text">
        {s.shortcuts
          .filter((x) => !["push_to_talk", "hands_free", "command", "dismiss"].includes(x.action))
          .map((x) => (
            <ShortcutRow key={x.action} sc={x} conflict={s.shortcuts.some((y) => y !== x && sig(y.keys) === sig(x.keys))} onChange={(k) => setKeys(x.action, k)} />
          ))}
      </Section>
    </Page>
  );
}
