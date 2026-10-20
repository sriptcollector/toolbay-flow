import { useEffect, useRef, useState } from "react";
import { api, Hardware, LANGUAGES, ModelRow, on } from "../api";
import { comboLabel } from "../keys";
import { Logo } from "./Hub";
import { ProgressBar, useProgress } from "./pages/Models";
import { useRecorder } from "./pages/Shortcuts";
import { fmtBytes, Keys, Toggle, useSettings } from "./ui";

const STEPS = ["Welcome", "Privacy", "Microphone", "Models", "Shortcut", "Language", "Try it"] as const;

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [s, save] = useSettings();
  if (!s) return null;
  const next = () => setStep((x) => Math.min(x + 1, STEPS.length - 1));
  const back = () => setStep((x) => Math.max(0, x - 1));
  const finish = async () => {
    await save({ onboarded: true, show_bar: true });
    onDone();
  };
  return (
    <div className="ob">
      <div className="ob-top">
        <Logo size={26} />
        <div className="ob-steps">
          {STEPS.map((t, i) => (
            <span key={t} className={i === step ? "cur" : i < step ? "done" : ""}>
              {t}
            </span>
          ))}
        </div>
      </div>
      <div className="ob-body">
        {step === 0 && <Welcome next={next} />}
        {step === 1 && <Privacy next={next} back={back} />}
        {step === 2 && <Mic next={next} back={back} />}
        {step === 3 && <ModelsStep next={next} back={back} />}
        {step === 4 && <ShortcutStep next={next} back={back} />}
        {step === 5 && (
          <div className="ob-card">
            <h1>Which language do you speak?</h1>
            <p>Pick one for the best accuracy, or let Flow detect it each time.</p>
            <div className="langs">
              {LANGUAGES.slice(0, 20).map(([c, n]) => (
                <button key={c} className={s.language === c ? "sel" : ""} onClick={() => save({ language: c })}>
                  {n}
                </button>
              ))}
            </div>
            {s.language !== "en" && s.speech_model.endsWith(".en") && (
              <p className="warn">Your speech model is English-only. Pick a multilingual model on the next screens or in Models.</p>
            )}
            <Nav back={back} next={next} />
          </div>
        )}
        {step === 6 && <TryIt back={back} finish={finish} />}
      </div>
    </div>
  );
}

function Nav({ back, next, nextLabel = "Continue", disabled }: { back?: () => void; next: () => void; nextLabel?: string; disabled?: boolean }) {
  return (
    <div className="ob-nav">
      {back ? (
        <button className="btn ghost" onClick={back}>
          Back
        </button>
      ) : (
        <span />
      )}
      <button className="btn primary big" disabled={disabled} onClick={next}>
        {nextLabel}
      </button>
    </div>
  );
}

function Welcome({ next }: { next: () => void }) {
  return (
    <div className="ob-card hero">
      <h1 className="display">Talk. It types.</h1>
      <p className="lead">
        Flow turns what you say into clean, punctuated text in any app: email, chat, docs, code, AI prompts. It runs entirely on this computer, it's free, and
        the code is open.
      </p>
      <ul className="ticks">
        <li>Hold a key, speak, let go. Your words appear where your cursor is.</li>
        <li>Fillers and false starts are removed, lists and punctuation added.</li>
        <li>No account, no limits, no subscription.</li>
      </ul>
      <Nav next={next} nextLabel="Get started" />
    </div>
  );
}

function Privacy({ next, back }: { next: () => void; back: () => void }) {
  const [s, save] = useSettings();
  if (!s) return null;
  return (
    <div className="ob-card">
      <h1>Your voice stays here</h1>
      <p>Speech recognition and cleanup run on your own hardware with open models. Flow has no servers and sends no analytics.</p>
      <div className="card">
        <div className="row">
          <div className="row-text">
            <div className="row-title">Context awareness</div>
            <div className="row-desc">Read the text near your cursor so dictation fits in. Never from password fields.</div>
          </div>
          <Toggle on={s.context_awareness} onChange={(v) => save({ context_awareness: v })} />
        </div>
        <div className="row">
          <div className="row-text">
            <div className="row-title">Keep a history of my dictations</div>
            <div className="row-desc">Stored on this computer so you can copy or fix past text.</div>
          </div>
          <Toggle on={s.data_policy !== "never_store"} onChange={(v) => save({ data_policy: v ? "store_normally" : "never_store" })} />
        </div>
      </div>
      <Nav back={back} next={next} />
    </div>
  );
}

function Mic({ next, back }: { next: () => void; back: () => void }) {
  const [s, save] = useSettings();
  const [mics, setMics] = useState<string[]>([]);
  const [lvl, setLvl] = useState<number[]>(Array(24).fill(0));
  const [heard, setHeard] = useState(false);
  const [err, setErr] = useState("");
  useEffect(() => {
    api.listMics().then(setMics);
    const off = on<number>("mic-level", (l) => {
      setLvl((a) => [...a.slice(1), l]);
      if (l > 0.15) setHeard(true);
    });
    return () => {
      off();
      api.micTest(false);
    };
  }, []);
  useEffect(() => {
    if (!s) return;
    api.micTest(false).then(() => api.micTest(true)).catch((e) => setErr(String(e)));
  }, [s?.microphone]);
  if (!s) return null;
  return (
    <div className="ob-card">
      <h1>Check your microphone</h1>
      <p>Say a few words. The bars should move as you talk. Built-in and wired mics are the most reliable.</p>
      <select value={s.microphone ?? ""} onChange={(e) => save({ microphone: e.target.value || null })}>
        <option value="">System default</option>
        {mics.map((m) => (
          <option key={m}>{m}</option>
        ))}
      </select>
      <div className={`meter ${heard ? "ok" : ""}`}>
        {lvl.map((l, i) => (
          <i key={i} style={{ transform: `scaleY(${Math.max(0.06, Math.min(1, l * 1.4))})` }} />
        ))}
      </div>
      {err && <p className="warn">{err}</p>}
      <p className="muted small">{heard ? "Sounds good." : "Waiting for sound…"}</p>
      <Nav back={back} next={next} nextLabel={heard ? "It works" : "Continue"} />
    </div>
  );
}

function ModelsStep({ next, back }: { next: () => void; back: () => void }) {
  const [s, save] = useSettings();
  const [hw, setHw] = useState<Hardware | null>(null);
  const [models, setModels] = useState<ModelRow[]>([]);
  const [speech, setSpeech] = useState("");
  const [llm, setLlm] = useState<string>("");
  const [phase, setPhase] = useState<"pick" | "downloading" | "ready" | "error">("pick");
  const [err, setErr] = useState("");
  const prog = useProgress();

  useEffect(() => {
    Promise.all([api.hardware(), api.listModels(), api.getSettings()]).then(([h, m, st]) => {
      setHw(h);
      setModels(m);
      const strong = h.nvidia || h.platform === "macos";
      const english = st.language === "en";
      setSpeech(strong ? "large-v3-turbo-q5" : english ? "small.en" : "small");
      setLlm(strong ? "qwen3-4b-instruct" : "qwen3-1.7b");
    });
  }, []);
  if (!s || !hw) return null;

  const total = models.filter((m) => m.id === speech || m.id === llm).reduce((a, m) => a + (m.downloaded ? 0 : m.size_mb), 0);

  const start = async () => {
    setPhase("downloading");
    setErr("");
    try {
      await save({ speech_model: speech, local_llm_model: llm || s.local_llm_model, cleanup_level: llm ? "medium" : "light" });
      await api.downloadEngines(llm ? ["whisper", "llama"] : ["whisper"]);
      await api.downloadModel(speech);
      if (llm) await api.downloadModel(llm);
      setPhase("ready");
      api.startEngines().catch(() => {});
    } catch (e) {
      setErr(String(e));
      setPhase("error");
    }
  };

  const speechModels = models.filter((m) => m.kind === "speech");
  const llms = models.filter((m) => m.kind === "llm");

  return (
    <div className="ob-card wide">
      <h1>Download the AI models</h1>
      <p>
        One-time download, then Flow works offline.{" "}
        {hw.nvidia ? "We found an NVIDIA GPU, so we picked the most accurate models." : `Picked for a ${hw.threads}-thread CPU. You can switch any time in Models.`}
      </p>
      <div className="pick">
        <label>
          Speech recognition
          <select value={speech} disabled={phase !== "pick"} onChange={(e) => setSpeech(e.target.value)}>
            {speechModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label} · {fmtBytes(m.size_mb)}
                {m.downloaded ? " · downloaded" : ""}
              </option>
            ))}
          </select>
        </label>
        <label>
          Cleanup and formatting
          <select value={llm} disabled={phase !== "pick"} onChange={(e) => setLlm(e.target.value)}>
            {llms.map((m) => (
              <option key={m.id} value={m.id}>
                {m.label} · {fmtBytes(m.size_mb)}
                {m.downloaded ? " · downloaded" : ""}
              </option>
            ))}
            <option value="">No AI cleanup (rules only, no download)</option>
          </select>
        </label>
      </div>
      {phase !== "pick" && (
        <div className="dl">
          {[prog["whisper-cpu"], prog["whisper-cuda"], prog["whisper-metal"], prog[speech], prog["llama-cpu"], prog["llama-vulkan"], prog["llama-metal"], prog[llm]].map(
            (p, i) => p && p.stage !== "done" && <div key={i}><span className="muted small">{p.id}</span><ProgressBar p={p} /></div>,
          )}
          {phase === "ready" && <p className="ok">All set. Models are on this computer.</p>}
          {phase === "error" && <p className="warn">{err}</p>}
        </div>
      )}
      <div className="ob-nav">
        <button className="btn ghost" onClick={back} disabled={phase === "downloading"}>
          Back
        </button>
        {phase === "pick" || phase === "error" ? (
          <button className="btn primary big" onClick={start}>
            {total ? `Download ${fmtBytes(total)}` : "Continue"}
          </button>
        ) : (
          <button className="btn primary big" disabled={phase !== "ready"} onClick={next}>
            {phase === "ready" ? "Continue" : "Downloading…"}
          </button>
        )}
      </div>
    </div>
  );
}

function ShortcutStep({ next, back }: { next: () => void; back: () => void }) {
  const [s, save] = useSettings();
  const [live, setLive] = useState<Set<number>>(new Set());
  const [ok, setOk] = useState(false);
  const [changing, setChanging] = useState(false);
  const ptt = s?.shortcuts.find((x) => x.action === "push_to_talk");
  const rec = useRecorder((keys) => {
    if (!s) return;
    if (changing) {
      save({ shortcuts: s.shortcuts.map((x) => (x.action === "push_to_talk" ? { ...x, keys: [...keys].sort((a, b) => a - b) } : x)) });
      setChanging(false);
      setOk(true);
    } else if (ptt && [...keys].sort().join() === [...ptt.keys].sort().join()) {
      setOk(true);
    }
    setTimeout(() => rec.start(), 50);
  });
  useEffect(() => {
    setLive(new Set(rec.keys));
  }, [rec.keys]);
  useEffect(() => {
    rec.start();
    return () => rec.stop();
  }, []);
  if (!s || !ptt) return null;
  return (
    <div className="ob-card">
      <h1>Your dictation key</h1>
      <p>
        Hold <b>{comboLabel(ptt.keys)}</b>, speak, and let go. Try pressing it now.
      </p>
      <div className={`bigkeys ${ok ? "ok" : ""}`}>
        <Keys keys={changing ? rec.keys : ptt.keys} live={live} />
      </div>
      <p className="muted small">{changing ? "Press the new combination, then let go." : ok ? "Perfect." : "The keys light up while you hold them."}</p>
      <button className="link" onClick={() => setChanging(true)}>
        Use a different shortcut
      </button>
      <div className="tips">
        <div>
          <b>Hands-free:</b> double-tap the key, talk as long as you like, tap again to finish.
        </div>
        <div>
          <b>Cancel:</b> press Esc while Flow is listening.
        </div>
      </div>
      <Nav back={back} next={next} />
    </div>
  );
}

function TryIt({ back, finish }: { back: () => void; finish: () => void }) {
  const [s] = useSettings();
  const [text, setText] = useState("");
  const ta = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    ta.current?.focus();
  }, []);
  const ptt = s?.shortcuts.find((x) => x.action === "push_to_talk");
  return (
    <div className="ob-card wide">
      <h1>Try it</h1>
      <p>
        Click in the box, hold <b>{ptt ? comboLabel(ptt.keys) : "your key"}</b> and read this out, ums and all:
      </p>
      <blockquote>“Hey um can we move the call to Thursday at 3, no actually 4, and I'll send the notes after question mark”</blockquote>
      <textarea ref={ta} className="tryit" rows={5} value={text} onChange={(e) => setText(e.target.value)} placeholder="Your words will appear here…" />
      <Nav back={back} next={finish} nextLabel="Start using Flow" />
    </div>
  );
}
