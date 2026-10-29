import { useEffect, useState } from "react";
import { api, AutoInfo, Hardware, ModelRow, on, Progress } from "../../api";
import { fmtBytes, Page, Row, Section, Toggle, useSettings } from "../ui";

/** Tracks download progress events by id. */
export function useProgress() {
  const [p, setP] = useState<Record<string, Progress>>({});
  useEffect(() => on<Progress>("download-progress", (x) => setP((all) => ({ ...all, [x.id]: x }))), []);
  return p;
}

/** Background model setup status ("Getting a faster speech model…"). */
export function useModelStatus() {
  const [st, setSt] = useState<{ busy: boolean; message: string }>({ busy: false, message: "" });
  useEffect(() => on<{ busy: boolean; message: string }>("model-status", setSt), []);
  return st;
}

export function ProgressBar({ p }: { p?: Progress }) {
  if (!p || p.stage === "done") return null;
  const pct = p.total ? Math.round((100 * p.done) / p.total) : 0;
  return (
    <div className="progress">
      <i style={{ transform: `scaleX(${p.stage === "extracting" ? 1 : pct / 100})` }} />
      <span>{p.stage === "extracting" ? "Unpacking…" : `${pct}% · ${(p.done / 1e6).toFixed(0)} / ${(p.total / 1e6).toFixed(0)} MB`}</span>
    </div>
  );
}

/** One progress bar for whatever is downloading right now. */
export function ActiveDownload() {
  const prog = useProgress();
  const live = Object.values(prog).find((p) => p.stage !== "done");
  return live ? <ProgressBar p={live} /> : null;
}

export default function Models() {
  const [s, save] = useSettings();
  const [models, setModels] = useState<ModelRow[]>([]);
  const [hw, setHw] = useState<Hardware | null>(null);
  const [auto, setAuto] = useState<AutoInfo | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const prog = useProgress();
  const status = useModelStatus();
  const load = () => {
    api.listModels().then(setModels);
    api.hardware().then(setHw);
    api.autoInfo().then(setAuto);
  };
  useEffect(load, []);
  useEffect(() => {
    // Engine status is re-read when the window gains focus or a download finishes, not polled.
    const refresh = () => api.hardware().then(setHw);
    window.addEventListener("focus", refresh);
    const off = on("settings-changed", load);
    const offDl = on<{ stage: string }>("download-progress", (p) => p.stage === "done" && refresh());
    return () => {
      window.removeEventListener("focus", refresh);
      off();
      offDl();
    };
  }, []);
  if (!s) return null;

  const label = (id?: string | null) => models.find((m) => m.id === id)?.label ?? id ?? "–";
  const usingAI = s.cleanup_level === "medium" || s.cleanup_level === "high";

  const get = async (m: ModelRow) => {
    setErr("");
    setBusy(m.id);
    try {
      await api.downloadEngines([m.kind === "speech" ? "whisper" : "llama"]);
      await api.downloadModel(m.id);
      await save(m.kind === "speech" ? { speech_model: m.id, model_mode: "manual" } : { local_llm_model: m.id, model_mode: "manual" });
    } catch (e) {
      setErr(`That download didn't finish. Check your internet connection and try again. (${e})`);
    } finally {
      setBusy(null);
      load();
    }
  };

  const list = (kind: "speech" | "llm") =>
    models
      .filter((m) => m.kind === kind)
      .map((m) => {
        const selected = kind === "speech" ? s.speech_model === m.id : s.local_llm_model === m.id;
        return (
          <div key={m.id} className={`model ${selected ? "sel" : ""}`}>
            <div className="model-main">
              <div className="model-name">
                {m.label} {selected && <span className="tag on">In use</span>}
              </div>
              <div className="model-note">
                {m.note} · {fmtBytes(m.size_mb)}
                {m.english_only ? " · English only" : ""}
              </div>
              <ProgressBar p={prog[m.id]} />
            </div>
            <div className="model-actions">
              {m.downloaded ? (
                <>
                  {!selected && (
                    <button className="btn" onClick={() => save(kind === "speech" ? { speech_model: m.id, model_mode: "manual" } : { local_llm_model: m.id, model_mode: "manual" })}>
                      Use
                    </button>
                  )}
                  {!selected && (
                    <button className="link danger" onClick={() => api.deleteModel(m.id).then(load)}>
                      Delete
                    </button>
                  )}
                </>
              ) : (
                <button className="btn" disabled={!!busy} onClick={() => get(m)}>
                  {busy === m.id ? "Downloading…" : "Download"}
                </button>
              )}
            </div>
          </div>
        );
      });

  return (
    <Page title="Models" sub="Toolbay Flow picks the AI models for this computer by itself, so dictation stays instant. Everything runs on this computer.">
      {err && <div className="alert">{err}</div>}
      <EngineChoice />
      <Section>
        <Row
          title={
            <>
              Model: {s.model_mode === "auto" ? "Auto" : "Manual"}{" "}
              <span className="muted">
                (currently {label(auto?.speech_in_use ?? s.speech_model)}
                {usingAI ? ` + ${label(s.local_llm_model)}` : ", quick cleanup"})
              </span>
            </>
          }
          desc={
            s.model_mode === "auto"
              ? `Chosen for ${s.auto_reason || auto?.choice.why || "this computer"}. Toolbay Flow re-checks when your hardware changes and steps down if dictation gets slow.`
              : "You picked the models yourself. Switch back to Auto to let Toolbay Flow choose."
          }
        >
          {s.model_mode === "manual" ? (
            <button className="btn" onClick={() => save({ model_mode: "auto" }).then(() => api.rerunAuto())}>
              Use Auto
            </button>
          ) : (
            <span className="status ok">Auto</span>
          )}
        </Row>
        {(status.busy || status.message) && (
          <div className="pad-row col">
            <span className="muted small">{status.message || "Getting models ready…"}</span>
            <ActiveDownload />
          </div>
        )}
      </Section>

      {hw && (
        <Section title="This computer">
          <Row title="Graphics card" desc={auto ? `${auto.hardware.gpu_name || "None found"}${auto.hardware.vram_mb ? ` · ${Math.round(auto.hardware.vram_mb / 1024)} GB` : ""} · ${Math.round((auto.hardware.ram_mb || 0) / 1024)} GB memory · ${auto.hardware.threads} threads` : ""}>
            <Toggle on={s.use_gpu} onChange={(v) => save({ use_gpu: v })} />
          </Row>
          <Row title="Speech engine" desc={hw.speech_engine ?? "Not available on this platform"}>
            <span className={`status ${hw.whisper_running ? "ok" : ""}`}>{hw.whisper_running ? "Running" : "Starts when needed"}</span>
          </Row>
          <Row title="Cleanup engine" desc={usingAI ? hw.cleanup_engine ?? "" : "Not needed for quick cleanup"}>
            <span className={`status ${hw.llama_running ? "ok" : ""}`}>{hw.llama_running ? "Running" : usingAI ? "Starts when needed" : "Off"}</span>
          </Row>
        </Section>
      )}

      <button className="disclosure" onClick={() => setAdvanced(!advanced)}>
        {advanced ? "▾" : "▸"} Advanced: choose models yourself
      </button>
      {advanced && (
        <>
          <Section title="Speech recognition" aside={<span className="muted small">Whisper models by OpenAI, MIT licence</span>}>
            <div className="models">{list("speech")}</div>
          </Section>
          <Section title="Cleanup and formatting" aside={<span className="muted small">Qwen models, Apache 2.0 licence</span>}>
            <div className="models">{list("llm")}</div>
          </Section>
          <div className="pad-row">
            <button className="link" onClick={() => api.openDataDir()}>
              Open model folder
            </button>
          </div>
        </>
      )}
    </Page>
  );
}

const PRESETS: Record<string, { label: string; base: string; asr: string; llm: string; keyUrl: string }> = {
  groq: { label: "Groq", base: "https://api.groq.com/openai/v1", asr: "whisper-large-v3-turbo", llm: "llama-3.3-70b-versatile", keyUrl: "https://console.groq.com/keys" },
  openai: { label: "OpenAI", base: "https://api.openai.com/v1", asr: "gpt-4o-transcribe", llm: "gpt-4o-mini", keyUrl: "https://platform.openai.com/api-keys" },
};

/** Local (default, private, free) vs hosted speech + cleanup with the user's own key. */
function EngineChoice() {
  const [s, save] = useSettings();
  const [preset, setPreset] = useState("groq");
  const [key, setKey] = useState("");
  if (!s) return null;
  const cloud = s.engine === "cloud";
  const useCloud = (k: string) => {
    const p = PRESETS[preset];
    return save({
      engine: "cloud",
      cloud_asr: { base_url: p.base, api_key: k, model: p.asr },
      cloud_llm: { base_url: p.base, api_key: k, model: p.llm },
      llm_provider: "open_ai_compatible",
      cleanup_level: s.cleanup_level === "none" || s.cleanup_level === "light" ? "medium" : s.cleanup_level,
    });
  };
  return (
    <Section title="Engine">
      <div className="engines">
        <button className={`engine ${!cloud ? "sel" : ""}`} onClick={() => save({ engine: "local", llm_provider: "local" })}>
          <b>On this computer</b>
          <span>Free. Your voice and text are processed on this computer, not sent to an AI provider. Fast on most PCs.</span>
          <em>Default</em>
        </button>
        <button className={`engine ${cloud ? "sel" : ""}`} onClick={() => !cloud && s.cloud_asr.api_key && useCloud(s.cloud_asr.api_key)}>
          <b>Cloud, with your own key</b>
          <span>The same approach paid dictation apps use: large hosted models for speech and cleanup. Fastest and most accurate on any computer.</span>
          <em>Uses your API key</em>
        </button>
      </div>
      <div className="pad">
        <p className="muted small">
          With the cloud engine, your recordings and text are sent to the provider you pick, using your own account. Toolbay doesn't see them. The provider has its own
          privacy policy and may charge for use (Groq has a free tier). If the cloud can't be reached, Flow uses the local engine for that dictation. Flow itself stays
          free.
        </p>
        <div className="subform cloudform">
          <label>
            Provider
            <select value={preset} onChange={(e) => setPreset(e.target.value)}>
              {Object.entries(PRESETS).map(([id, p]) => (
                <option key={id} value={id}>
                  {p.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            API key{" "}
            <a className="link small" href={PRESETS[preset].keyUrl} target="_blank" rel="noreferrer">
              Get a key
            </a>
            <input type="password" placeholder={cloud ? "Saved. Paste a new one to replace it" : "Paste your API key"} value={key} onChange={(e) => setKey(e.target.value)} />
          </label>
          <label>
            &nbsp;
            <button className="btn primary" disabled={!key.trim()} onClick={() => useCloud(key.trim()).then(() => setKey(""))}>
              Use cloud engine
            </button>
          </label>
        </div>
        {cloud && (
          <p className="muted small">
            Speech: {s.cloud_asr.model} · Cleanup: {s.cloud_llm.model} at {s.cloud_asr.base_url.replace("https://", "")}
          </p>
        )}
      </div>
    </Section>
  );
}
