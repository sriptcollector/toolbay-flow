import { useEffect, useState } from "react";
import { ArrowRight, Trash2 } from "lucide-react";
import { api, DictEntry, on } from "../../api";
import { Empty, Modal, Tabs } from "../ui";

const MAX = 4000;
const EXAMPLES: [string, string][] = [
  ["my email", "you@example.com"],
  ["my calendar link", "cal.com/you/30min"],
  ["my address", "221B Baker Street, London"],
];
type Filter = "all" | "used";

export default function Snippets() {
  const [list, setList] = useState<DictEntry[]>([]);
  const [tab, setTab] = useState<Filter>("all");
  const [edit, setEdit] = useState<Partial<DictEntry> | null>(null);
  const load = () => api.dictList(true).then(setList);
  useEffect(() => {
    load();
    return on("dictionary-changed", load);
  }, []);

  const save = async () => {
    if (!edit?.phrase?.trim() || !edit.replacement?.trim()) return;
    await api.dictSave({ ...edit, is_snippet: true, phrase: edit.phrase.trim() });
    setEdit(null);
    load();
  };

  const shown = tab === "used" ? [...list].filter((e) => e.uses > 0).sort((a, b) => b.uses - a.uses) : list;

  return (
    <div className="page">
      <header className="page-head">
        <h1>Snippets</h1>
        <button className="btn primary" onClick={() => setEdit({ phrase: "", replacement: "" })}>
          Add new
        </button>
      </header>
      <Tabs
        tabs={[
          ["all", "All"],
          ["used", "Most used"],
        ]}
        value={tab}
        onChange={setTab}
      />
      <section className="hero night">
        <h2>Say it once, type it never</h2>
        <p>
          Save the things you type over and over: emails, links, addresses, sign-offs. <b>Say the cue and Toolbay Flow writes the whole thing.</b>
        </p>
        <div className="snip-examples">
          {EXAMPLES.map(([cue, text]) => (
            <div key={cue} className="snip-ex">
              <span className="btn glass cue">“{cue}”</span>
              <ArrowRight size={16} />
              <span className="btn glass">{text}</span>
            </div>
          ))}
        </div>
        <div className="hero-actions">
          <button className="btn on-dark" onClick={() => setEdit({ phrase: "", replacement: "" })}>
            Add a new snippet
          </button>
        </div>
      </section>

      {shown.length === 0 ? (
        <Empty title={tab === "all" ? "No snippets yet" : "No snippets used yet"}>Try one for your email signature: say “my signature” and get the whole block.</Empty>
      ) : (
        <div className="list-card">
          {shown.map((e) => (
            <div key={e.id} className="entry" onClick={() => setEdit(e)}>
              <div className="entry-main">
                {e.phrase} <span className="arrow">→</span> {e.replacement?.replace(/\s+/g, " ")}
              </div>
              {e.uses > 0 && <span className="uses">{e.uses}×</span>}
              <div className="entry-tools">
                <button
                  className="icon-btn"
                  title="Delete"
                  onClick={(ev) => {
                    ev.stopPropagation();
                    api.dictDelete(e.id).then(load);
                  }}
                >
                  <Trash2 />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {edit && (
        <Modal
          title={edit.id ? "Edit snippet" : "New snippet"}
          onClose={() => setEdit(null)}
          actions={
            <>
              <button className="btn" onClick={() => setEdit(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={save} disabled={!edit.phrase?.trim() || !edit.replacement?.trim()}>
                Save snippet
              </button>
            </>
          }
        >
          <label>
            When I say
            <input autoFocus placeholder="my calendar link" value={edit.phrase ?? ""} onChange={(e) => setEdit({ ...edit, phrase: e.target.value })} />
          </label>
          <label>
            Write this
            <textarea rows={5} maxLength={MAX} placeholder="https://cal.com/you/30min" value={edit.replacement ?? ""} onChange={(e) => setEdit({ ...edit, replacement: e.target.value })} />
            <span className="count">
              {(edit.replacement ?? "").length}/{MAX}
            </span>
          </label>
        </Modal>
      )}
    </div>
  );
}
