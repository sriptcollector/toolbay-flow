import { useState } from "react";
import { api } from "../api";
import { Modal } from "./ui";

type Kind = "feedback" | "bug" | "idea" | "showoff";
const KINDS: [Kind, string, string][] = [
  ["feedback", "Feedback", "What do you think of Toolbay Flow?"],
  ["bug", "Something's broken", "What happened, and what did you expect? The more detail, the faster it gets fixed."],
  ["idea", "Idea", "What should Toolbay Flow do next?"],
  ["showoff", "Show off", "Tell people what you're doing with Toolbay Flow."],
];

/** Sends feedback to the Toolbay Flow space on toolbay.ai (needs a Toolbay sign-in). */
export default function Feedback({ onClose }: { onClose: () => void }) {
  const [kind, setKind] = useState<Kind>("feedback");
  const [text, setText] = useState("");
  const [debug, setDebug] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [needSignIn, setNeedSignIn] = useState(false);
  const [sent, setSent] = useState<string | null>(null);
  const priv = kind === "bug" && debug;

  const send = async () => {
    setErr("");
    setNeedSignIn(false);
    setBusy(true);
    try {
      setSent(await api.sendFeedback(kind, text, kind === "bug" && debug, !priv));
    } catch (e) {
      const m = String(e);
      if (m === "signin") setNeedSignIn(true);
      else setErr(m);
    } finally {
      setBusy(false);
    }
  };

  if (sent)
    return (
      <Modal
        title="Sent. Thanks!"
        onClose={onClose}
        actions={
          <>
            <a className="btn" href={sent} target="_blank" rel="noreferrer">
              See it on toolbay.ai
            </a>
            <button className="btn primary" onClick={onClose}>
              Done
            </button>
          </>
        }
      >
        <p className="muted" style={{ margin: 0 }}>
          {priv ? "Only the maker of Toolbay Flow can see this." : "It's in the Toolbay Flow community, where others can add to it."}
        </p>
      </Modal>
    );

  return (
    <Modal
      title="Share feedback"
      onClose={onClose}
      actions={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" onClick={send} disabled={busy || text.trim().length < 3}>
            {busy ? "Sending…" : "Send"}
          </button>
        </>
      }
    >
      <div className="chips fb-kinds">
        {KINDS.map(([id, label]) => (
          <button key={id} className={kind === id ? "sel" : ""} onClick={() => setKind(id)}>
            {label}
          </button>
        ))}
      </div>
      <textarea autoFocus rows={6} maxLength={5000} placeholder={KINDS.find((k) => k[0] === kind)![2]} value={text} onChange={(e) => setText(e.target.value)} />
      {kind === "bug" && (
        <label className="check">
          <input type="checkbox" checked={debug} onChange={(e) => setDebug(e.target.checked)} /> Include debug info (only the maker sees it)
        </label>
      )}
      <p className="muted small" style={{ margin: 0 }}>
        {priv
          ? "Sent to the maker on toolbay.ai (not posted publicly) with your app version, models and recent warnings. Never your dictations, recordings or keys."
          : "Posted in the Toolbay Flow community on toolbay.ai under your Toolbay name."}
      </p>
      {needSignIn && (
        <div className="alert">
          Sign in to Toolbay to send feedback.{" "}
          <button className="link" onClick={() => api.syncSignIn().catch((e) => setErr(String(e)))}>
            Sign in
          </button>
        </div>
      )}
      {err && <div className="alert">{err}</div>}
    </Modal>
  );
}
