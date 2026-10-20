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
}

const BARS = 12;

export default function Bar() {
  const [st, setSt] = useState<BarState>({ phase: "idle", mode: null, message: null });
  const [bubble, setBubble] = useState<{ text: string; kind: string } | null>(null);
  const [hover, setHover] = useState(false);
  const [hint, setHint] = useState("");
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
        const ptt = s.shortcuts.find((x) => x.action === "push_to_talk");
        setHint(ptt ? comboLabel(ptt.keys) : "");
      });
    loadHint();
    const offs = [
      on<BarState>("bar-state", (s) => {
        window.clearTimeout(doneTimer.current);
        if (s.phase === "notice" || s.phase === "learned" || s.phase === "error") {
          showBubble(s.message ?? "", s.phase);
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
    ];
    return () => offs.forEach((f) => f());
  }, []);

  function showBubble(text: string, kind: string) {
    window.clearTimeout(bubbleTimer.current);
    setBubble({ text, kind });
    bubbleTimer.current = window.setTimeout(() => setBubble(null), kind === "learned" ? 6000 : 3200);
  }

  // Report interactive areas so the rest of the window stays click-through.
  useLayoutEffect(() => {
    const rects: number[][] = [];
    const add = (el: HTMLElement | null, pad: number) => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      rects.push([r.left - pad, r.top - pad, r.width + pad * 2, r.height + pad * 2]);
    };
    add(pillRef.current, st.phase === "idle" && !hover ? 10 : 6);
    add(bubbleRef.current, 2);
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
        </div>
      )}
      {hover && st.phase === "idle" && !bubble && hint && (
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
          api.barAction("open");
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
              const bulge = 1 - ((i - c) * (i - c)) / (c * c * 1.6);
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
        {st.phase === "idle" && hover && <div className="mic" />}
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
