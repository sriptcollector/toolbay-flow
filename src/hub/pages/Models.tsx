import { useEffect, useState } from "react";
import { api, Hardware, ModelRow, on, Progress } from "../../api";
import { fmtBytes, Page, Row, Section, Toggle, useSettings } from "../ui";

/** Tracks download progress events by id. */
export function useProgress() {
  const [p, setP] = useState<Record<string, Progress>>({});
  useEffect(() => on<Progress>("download-progress", (x) => setP((all) => ({ ...all, [x.id]: x }))), []);
  return p;
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

export default function Models() {
  const [s, save] = useSettings();
  const [models, setModels] = useState<ModelRow[]>([]);
  const [hw, setHw] = useState<Hardware | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const prog = useProgress();
  const load = () => {
    api.listModels().then(setModels);
    api.hardware().then(setHw);
  };
  useEffect(load, []);
  useEffect(() => {
    const t = setInterval(() => api.hardware().then(setHw), 3000);
    return () => clearInterval(t);
  }, []);
  if (!s) return null;

  const get = async (m: ModelRow, select: boolean) => {
    setErr("");
    setBusy(m.id);
    try {
      await api.downloadEngines([m.kind === "speech" ? "whisper" : "llama"]);
      await api.downloadModel(m.id);
      if (select) await save(m.kind === "speech" ? { speech_model: m.id } : { local_llm_model: m.id });
    } catch (e) {
      setErr(String(e));
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
                    <button className="btn" onClick={() => save(kind === "speech" ? { speech_model: m.id } : { local_llm_model: m.id })}>
                      Use
                    </button>
                  )}
                  <button className="link danger" onClick={() => api.deleteModel(m.id).then(load)}>
                    Delete
                  </button>
                </>
              ) : (
                <button className="btn" disabled={!!busy} onClick={() => get(m, true)}>
                  {busy === m.id ? "Downloading…" : "Download"}
                </button>
              )}
            </div>
          </div>
        );
      });

  return (
    <Page title="Models" sub="Everything runs on this computer. Models download once from Hugging Face and the engines from the open-source whisper.cpp and llama.cpp releases.">
      {err && <div className="alert">{err}</div>}
      {hw && (
        <Section title="This computer">
          <Row title="Graphics acceleration" desc={hw.nvidia ? "NVIDIA GPU found. Speech and cleanup run on the GPU." : "Uses Vulkan for cleanup when a GPU is available; speech runs on the CPU."}>
            <Toggle on={s.use_gpu} onChange={(v) => save({ use_gpu: v })} />
          </Row>
          <Row title="Speech engine" desc={hw.speech_engine ?? "Not available on this platform"}>
            <span className={`status ${hw.whisper_running ? "ok" : ""}`}>{hw.whisper_running ? "Running" : hw.speech_engine_ready ? "Ready" : "Not downloaded"}</span>
          </Row>
          <Row title="Cleanup engine" desc={hw.cleanup_engine ?? "Not available on this platform"}>
            <span className={`status ${hw.llama_running ? "ok" : ""}`}>{hw.llama_running ? "Running" : hw.cleanup_engine_ready ? "Ready" : "Not downloaded"}</span>
          </Row>
          <ProgressBar p={prog["whisper-cpu"] ?? prog["whisper-cuda"] ?? prog["whisper-metal"]} />
          <ProgressBar p={prog["llama-cpu"] ?? prog["llama-vulkan"] ?? prog["llama-metal"]} />
          <div className="pad-row">
            <button className="btn" onClick={() => api.startEngines().then(load).catch((e) => setErr(String(e)))}>
              Start engines now
            </button>
            <button className="link" onClick={() => api.openDataDir()}>
              Open model folder
            </button>
          </div>
        </Section>
      )}
      <Section title="Speech recognition" aside={<span className="muted small">Whisper models by OpenAI, MIT licence</span>}>
        <div className="models">{list("speech")}</div>
      </Section>
      <Section title="Cleanup and formatting" aside={<span className="muted small">Qwen models, Apache 2.0 licence</span>}>
        <div className="models">{list("llm")}</div>
      </Section>
    </Page>
  );
}
