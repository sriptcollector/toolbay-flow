import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api, on, Settings } from "../api";
import { comboLabel } from "../keys";
import "./bar.css";

type Phase = "idle" | "recording" | "processing" | "done" | "error" | "notice" | "learned";
type Mode = "ptt" | "hands_free" | "command";
interface BarState {
  phase: Phase;
  mode: Mode | null;
  message: string | null;
  /** Optional button: [label, bar action id]. */
  action?: [string, string] | null;
  ms?: number | null;
}

const BARS = 10;

export default function Bar() {
  const [st, setSt] = useState<BarState>({ phase: "idle", mode: null, message: null });
  const [bubble, setBubble] = useState<{ text: string; kind: string; action?: [string, string] | null } | null>(null);
  const [hover, setHover] = useState(false);
  const [hint, setHint] = useState("");
  const [menu, setMenu] = useState(false);
  const [mics, setMics] = useState<string[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  // "Hide for 1 hour": the pill goes away, messages above it still show.
  const [snoozed, setSnoozed] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const levels = useRef<number[]>(Array(BARS).fill(0));
  const [, force] = useState(0);
  const pillRef = useRef<HTMLDivElement>(null);
  const bubbleRef = useRef<HTMLDivElement>(null);
  const bubbleTimer = useRef<number | undefined>(undefined);
  const doneTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    document.documentElement.classList.add("bar-root");
    const loadHint = () =>
      api.getSettings().then((s: Settings) => {
        setSettings(s);
        setSnoozed(s.bar_snooze_until * 1000 > Date.now());
        const ptt = s.shortcuts.find((x) => x.action === "push_to_talk");
        setHint(ptt ? comboLabel(ptt.keys) : "");
      });
    loadHint();
    const offs = [
      on<BarState>("bar-state", (s) => {
        window.clearTimeout(doneTimer.current);
        if (s.phase === "notice" || s.phase === "learned" || s.phase === "error") {
          showBubble(s.message ?? "", s.phase, s.action, s.ms);
          if (s.phase !== "error") return;
          setSt({ phase: "error", mode: null, message: s.message });
          doneTimer.current = window.setTimeout(() => setSt({ phase: "idle", mode: null, message: null }), 1600);
          return;
        }
        if (s.phase === "recording") levels.current = Array(BARS).fill(0);
        setSt(s);
        if (s.phase === "done") {
          doneTimer.current = window.setTimeout(() => setSt({ phase: "idle", mode: null, message: null }), 1100);
        }
      }),
      on<number>("bar-level", (l) => {
        const arr = levels.current;
        arr.shift();
        arr.push(l);
        force((n) => n + 1);
      }),
      on("settings-changed", loadHint),
      on<number>("bar-snoozed", (until) => setSnoozed(until * 1000 > Date.now())),
    ];
    return () => offs.forEach((f) => f());
  }, []);

  function showBubble(text: string, kind: string, action?: [string, string] | null, ms?: number | null) {
    window.clearTimeout(bubbleTimer.current);
    setBubble({ text, kind, action });
    bubbleTimer.current = window.setTimeout(() => setBubble(null), ms ?? (kind === "learned" ? 6000 : 3200));
  }

  // Report interactive areas so the rest of the window stays click-through.
  useLayoutEffect(() => {
    const rects: number[][] = [];
    const add = (el: HTMLElement | null, pad: number) => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      rects.push([r.left - pad, r.top - pad, r.width + pad * 2, r.height + pad * 2]);
    };
    if (!(snoozed && st.phase === "idle")) add(pillRef.current, st.phase === "idle" && !hover ? 10 : 6);
    add(bubbleRef.current, 2);
    add(menuRef.current, 2);
    api.barHit(rects).catch(() => {});
  });

  const recording = st.phase === "recording";
  const expanded = hover || st.phase !== "idle";
  const cls = [
    "pill",
    expanded ? "open" : "rest",
    st.phase,
    st.mode === "command" ? "command" : "",
    st.mode === "hands_free" && recording ? "hf" : "",
    // Snoozed: only the resting pill hides; dictation still shows.
    snoozed && st.phase === "idle" && !menu ? "snoozed" : "",
  ].join(" ");

  return (
    <div className="bar-stage">
      {bubble && (
        <div ref={bubbleRef} className={`bubble ${bubble.kind}`}>
          <span>{bubble.text}</span>
          {bubble.kind === "learned" && (
            <button
              onClick={() => {
                api.undoLearned();
                setBubble(null);
              }}
            >
              Undo
            </button>
          )}
          {bubble.action && (
            <button
              onClick={() => {
                api.barAction(bubble.action![1]);
                setBubble(null);
              }}
            >
              {bubble.action[0]}
            </button>
          )}
        </div>
      )}
      {menu && settings && (
        <div ref={menuRef} className="menu" onMouseLeave={() => setMenu(false)}>
          {/* Same order as the original's bar menu. */}
          <button onClick={() => (setMenu(false), api.barAction("hide_1h"))}>Hide for 1 hour</button>
          <button onClick={() => (setMenu(false), api.barAction("settings"))}>Settings</button>
          <div className="menu-sep" />
          <div className="menu-label">Microphone</div>
          {[null, ...mics].map((m) => (
            <button
              key={m ?? "default"}
              className={settings.microphone === m ? "on" : ""}
              onClick={() => {
                setSettings({ ...settings, microphone: m });
                api.setMicrophone(m);
                setMenu(false);
              }}
            >
              {m ?? "System default"}
            </button>
          ))}
          <div className="menu-sep" />
          <button onClick={() => (setMenu(false), api.barAction("history"))}>Dictation history</button>
          <button onClick={() => (setMenu(false), api.barAction("paste_last"))}>Paste last dictation</button>
        </div>
      )}
      {hover && st.phase === "idle" && !bubble && !menu && hint && (
        <div className="tip">
          Hold <b>{hint}</b> to dictate · click to start hands-free
        </div>
      )}
      <div
        ref={pillRef}
        className={cls}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onClick={() => {
          if (st.phase === "idle") api.barAction("start_handsfree");
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          api.listMics().then(setMics);
          setMenu(!menu);
        }}
      >
        {recording && st.mode === "hands_free" && (
          <button
            className="ctl cancel"
            title="Cancel"
            onClick={(e) => {
              e.stopPropagation();
              api.barAction("cancel");
            }}
          >
            <svg viewBox="0 0 10 10"><path d="M2 2l6 6M8 2l-6 6" /></svg>
          </button>
        )}
        {recording && (
          <div className="wave">
            {levels.current.map((l, i) => {
              const c = (BARS - 1) / 2;
              const bulge = 1 - ((i - c) * (i - c)) / 48;
              const h = Math.max(0.12, Math.min(1, l * 1.6 * bulge + 0.08));
              return <i key={i} style={{ transform: `scaleY(${h})` }} />;
            })}
          </div>
        )}
        {recording && st.mode === "command" && <span className="label">Command</span>}
        {st.phase === "processing" && (
          <div className="dots">
            <i />
            <i />
            <i />
            {st.message && <span className="label">{st.message}</span>}
          </div>
        )}
        {st.phase === "done" && (
          <div className="done">
            <svg viewBox="0 0 12 12"><path d="M2.5 6.5l2.2 2.2L9.5 3.5" /></svg>
            {st.message && <span className="label">{st.message}</span>}
          </div>
        )}
        {st.phase === "error" && <span className="label err">!</span>}
        {st.phase === "idle" && hover && (
          <div className="wave flat">
            {Array.from({ length: BARS }).map((_, i) => (
              <i key={i} />
            ))}
          </div>
        )}
        {recording && st.mode === "hands_free" && (
          <button
            className="ctl stop"
            title="Finish"
            onClick={(e) => {
              e.stopPropagation();
              api.barAction("stop");
            }}
          >
            <span />
          </button>
        )}
      </div>
    </div>
  );
}
