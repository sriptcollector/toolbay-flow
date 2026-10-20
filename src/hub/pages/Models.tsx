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
    const t = setInterval(() => api.hardware().then(setHw), 3000);
    const off = on("settings-changed", load);
    return () => {
      clearInterval(t);
      off();
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
    <Page title="Models" sub="Flow picks the AI models for this computer by itself, so dictation stays instant. Everything runs on this computer.">
      {err && <div className="alert">{err}</div>}
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
              ? `Chosen for ${s.auto_reason || auto?.choice.why || "this computer"}. Flow re-checks when your hardware changes and steps down if dictation gets slow.`
              : "You picked the models yourself. Switch back to Auto to let Flow choose."
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
