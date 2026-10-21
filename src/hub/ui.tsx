import { ReactNode, useCallback, useEffect, useState } from "react";
import { api, on, Settings } from "../api";
import { sortKeys, keyName } from "../keys";

/** Shared settings state with optimistic save. */
export function useSettings(): [Settings | null, (patch: Partial<Settings>) => Promise<void>, () => void] {
  const [s, setS] = useState<Settings | null>(null);
  const reload = useCallback(() => {
    api.getSettings().then(setS);
  }, []);
  useEffect(() => {
    reload();
    return on("settings-changed", reload);
  }, [reload]);
  const save = useCallback(
    async (patch: Partial<Settings>) => {
      const cur = await api.getSettings();
      const next = { ...cur, ...patch };
      setS(next);
      await api.saveSettings(next);
    },
    [],
  );
  return [s, save, reload];
}

export function Toggle({ on: value, onChange, disabled }: { on: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      role="switch"
      aria-checked={value}
      disabled={disabled}
      className={`toggle ${value ? "on" : ""}`}
      onClick={() => onChange(!value)}
    >
      <span />
    </button>
  );
}

export function Row({ title, desc, children }: { title: ReactNode; desc?: ReactNode; children?: ReactNode }) {
  return (
    <div className="row">
      <div className="row-text">
        <div className="row-title">{title}</div>
        {desc && <div className="row-desc">{desc}</div>}
      </div>
      <div className="row-ctl">{children}</div>
    </div>
  );
}

export function Section({ title, children, aside }: { title?: ReactNode; children: ReactNode; aside?: ReactNode }) {
  return (
    <section className="section">
      {(title || aside) && (
        <div className="section-head">
          {title && <h3>{title}</h3>}
          {aside}
        </div>
      )}
      <div className="card">{children}</div>
    </section>
  );
}

export function Page({ title, sub, children, actions }: { title: string; sub?: ReactNode; children: ReactNode; actions?: ReactNode }) {
  return (
    <div className="page">
      <header className="page-head">
        <div>
          <h1>{title}</h1>
          {sub && <p className="sub">{sub}</p>}
        </div>
        {actions && <div className="page-actions">{actions}</div>}
      </header>
      {children}
    </div>
  );
}

export function Keys({ keys, live }: { keys: number[]; live?: Set<number> }) {
  return (
    <span className="keys">
      {sortKeys(keys).map((k) => (
        <kbd key={k} className={live?.has(k) ? "lit" : ""}>
          {keyName(k)}
        </kbd>
      ))}
    </span>
  );
}

export function Empty({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <div className="empty-title">{title}</div>
      {children && <div className="empty-body">{children}</div>}
    </div>
  );
}

export function fmtBytes(mb: number) {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

export function timeAgo(iso: string) {
  const d = new Date(iso);
  return d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function dayLabel(iso: string) {
  const d = new Date(iso);
  const today = new Date();
  const y = new Date();
  y.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === y.toDateString()) return "Yesterday";
  return d.toLocaleDateString([], { weekday: "long", month: "short", day: "numeric" });
}

export const Icon = {
  home: <svg viewBox="0 0 20 20"><path d="M3 9.5L10 4l7 5.5V16a1 1 0 0 1-1 1h-3.5v-4.5h-5V17H4a1 1 0 0 1-1-1z" /></svg>,
  chart: <svg viewBox="0 0 20 20"><path d="M4 16V9M10 16V4M16 16v-5" /></svg>,
  book: <svg viewBox="0 0 20 20"><path d="M4 4.5A1.5 1.5 0 0 1 5.5 3H16v12H5.5A1.5 1.5 0 0 0 4 16.5zM4 16.5A1.5 1.5 0 0 0 5.5 18H16" /></svg>,
  bolt: <svg viewBox="0 0 20 20"><path d="M11 2.5L4.5 11H10l-1 6.5L15.5 9H10z" /></svg>,
  brush: <svg viewBox="0 0 20 20"><path d="M13.5 3.5l3 3-7 7-3-3zM6.5 10.5c-2 0-3 1.5-3 3.5 0 1-.5 1.5-1 2 3 .5 6-.5 6-3.5" /></svg>,
  wand: <svg viewBox="0 0 20 20"><path d="M4 16l9-9M12 4l.6 1.4L14 6l-1.4.6L12 8l-.6-1.4L10 6l1.4-.6zM16 9l.4 1 1 .4-1 .4-.4 1-.4-1-1-.4 1-.4z" /></svg>,
  note: <svg viewBox="0 0 20 20"><path d="M5 3h7l3 3v11H5zM12 3v3h3M8 10h4M8 13h4" /></svg>,
  chip: <svg viewBox="0 0 20 20"><path d="M6 6h8v8H6zM8 3v3M12 3v3M8 14v3M12 14v3M3 8h3M3 12h3M14 8h3M14 12h3" /></svg>,
  keyboard: <svg viewBox="0 0 20 20"><path d="M2.5 6h15v9h-15zM5 9h1M8 9h1M11 9h1M14 9h1M6 12h8" /></svg>,
  gear: <svg viewBox="0 0 20 20"><path d="M10 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM10 2.5v2M10 15.5v2M2.5 10h2M15.5 10h2M4.7 4.7l1.4 1.4M13.9 13.9l1.4 1.4M4.7 15.3l1.4-1.4M13.9 6.1l1.4-1.4" /></svg>,
  copy: <svg viewBox="0 0 20 20"><path d="M7 7h9v10H7zM4 13V3h9" /></svg>,
  trash: <svg viewBox="0 0 20 20"><path d="M4 6h12M8 6V4h4v2M6 6l1 11h6l1-11" /></svg>,
  retry: <svg viewBox="0 0 20 20"><path d="M15.5 8A6 6 0 1 0 16 12M16 3.5V8h-4.5" /></svg>,
  star: <svg viewBox="0 0 20 20"><path d="M10 3l2.1 4.4 4.9.6-3.6 3.3 1 4.8L10 13.8l-4.4 2.3 1-4.8L3 8l4.9-.6z" /></svg>,
  plus: <svg viewBox="0 0 20 20"><path d="M10 4v12M4 10h12" /></svg>,
  mic: <svg viewBox="0 0 20 20"><path d="M10 3a2.5 2.5 0 0 0-2.5 2.5v4a2.5 2.5 0 0 0 5 0v-4A2.5 2.5 0 0 0 10 3zM5 9.5a5 5 0 0 0 10 0M10 14.5V17" /></svg>,
  check: <svg viewBox="0 0 20 20"><path d="M4.5 10.5l3.5 3.5 7.5-8" /></svg>,
};

/** A centered dialog over a dimmed window. Esc or a click outside closes it. */
export function Modal({ title, children, onClose, actions }: { title: string; children: ReactNode; onClose: () => void; actions?: ReactNode }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="modal-back" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-label={title}>
        <h3>{title}</h3>
        {children}
        {actions && <div className="modal-actions">{actions}</div>}
      </div>
    </div>
  );
}

/** Underlined tabs, like the original's page tabs. */
export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: [T, string][]; value: T; onChange: (v: T) => void }) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map(([id, label]) => (
        <button key={id} role="tab" aria-selected={value === id} className={value === id ? "on" : ""} onClick={() => onChange(id)}>
          {label}
        </button>
      ))}
    </div>
  );
}

/** Short message at the bottom of the hub (like the original's toasts). */
export function toast(text: string) {
  window.dispatchEvent(new CustomEvent("flow-toast", { detail: text }));
}

export function Toaster() {
  const [msg, setMsg] = useState<{ text: string; n: number } | null>(null);
  useEffect(() => {
    let t: number | undefined;
    const show = (e: Event) => {
      window.clearTimeout(t);
      setMsg((m) => ({ text: (e as CustomEvent<string>).detail, n: (m?.n ?? 0) + 1 }));
      t = window.setTimeout(() => setMsg(null), 3000);
    };
    window.addEventListener("flow-toast", show);
    return () => {
      window.removeEventListener("flow-toast", show);
      window.clearTimeout(t);
    };
  }, []);
  return msg ? (
    <div key={msg.n} className="toast" role="status">
      {msg.text}
    </div>
  ) : null;
}

/** "Import" button: pick a file, preview what would be added, then add it. */
export function ImportButton({ snippets, onDone }: { snippets: boolean; onDone: () => void }) {
  const [preview, setPreview] = useState<import("../api").ImportPreview | null>(null);
  const pick = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = snippets ? ".json" : ".csv";
    input.onchange = async () => {
      const f = input.files?.[0];
      if (!f) return;
      setPreview(await api.dictImportPreview(await f.text(), snippets));
    };
    input.click();
  };
  const noun = snippets ? "snippet" : "word";
  const n = preview?.items.length ?? 0;
  return (
    <>
      <button className="btn" onClick={pick}>
        Import
      </button>
      {preview && (
        <Modal
          title={snippets ? "Import snippets" : "Import dictionary"}
          onClose={() => setPreview(null)}
          actions={
            <>
              <button className="btn" onClick={() => setPreview(null)}>
                Cancel
              </button>
              <button
                className="btn primary"
                disabled={n === 0}
                onClick={async () => {
                  const added = await api.dictImport(preview.items, snippets);
                  setPreview(null);
                  toast(`Added ${added} ${noun}${added === 1 ? "" : "s"}`);
                  onDone();
                }}
              >
                Add {n} {noun}
                {n === 1 ? "" : "s"}
              </button>
            </>
          }
        >
          {preview.error ? (
            <p className="muted">{preview.error}</p>
          ) : (
            <>
              <p className="muted">
                {n} new {noun}
                {n === 1 ? "" : "s"} found
                {preview.duplicates > 0 && `, ${preview.duplicates} already here`}
                {preview.skipped > 0 && `, ${preview.skipped} skipped (unreadable, repeated or too long)`}.
              </p>
              <div className="import-list">
                {preview.items.slice(0, 50).map((i) => (
                  <div key={i.phrase}>
                    {i.phrase}
                    {i.replacement && <span className="arrow"> → {i.replacement.replace(/\s+/g, " ")}</span>}
                  </div>
                ))}
              </div>
            </>
          )}
          <p className="muted small">{snippets ? 'A .json file holding a list of {"name": …, "text": …}.' : "A .csv file: one word per line, or heard,written for a replacement."}</p>
        </Modal>
      )}
    </>
  );
}
