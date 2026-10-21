import { useEffect, useState } from "react";
import { Star, Trash2, Sparkles } from "lucide-react";
import { api, DictEntry, on } from "../../api";
import { Empty, Modal, Row, Section, Tabs, Toggle, useSettings } from "../ui";

const EXAMPLES = ["Toolbay", "Kubernetes", "Q4 planning", "Siobhan"];
type Filter = "all" | "starred" | "learned";

export default function Dictionary() {
  const [list, setList] = useState<DictEntry[]>([]);
  const [tab, setTab] = useState<Filter>("all");
  const [adding, setAdding] = useState<{ word: string; from: string; rule: boolean } | null>(null);
  const [settings, save] = useSettings();
  const load = () => api.dictList(false).then(setList);
  useEffect(() => {
    load();
    const a = on("dictionary-learned", load);
    const b = on("dictionary-changed", load);
    return () => {
      a();
      b();
    };
  }, []);

  const add = async () => {
    if (!adding?.word.trim()) return;
    if (adding.rule && adding.from.trim()) await api.dictSave({ phrase: adding.from.trim(), replacement: adding.word.trim() });
    else await api.dictSave({ phrase: adding.word.trim() });
    setAdding(null);
    load();
  };
  const open = (word = "") => setAdding({ word, from: "", rule: false });

  const shown = list.filter((e) => (tab === "starred" ? e.starred : tab === "learned" ? e.source === "user_edits" : true));

  return (
    <div className="page">
      <header className="page-head">
        <h1>Dictionary</h1>
        <button className="btn primary" onClick={() => open()}>
          Add new
        </button>
      </header>
      <Tabs
        tabs={[
          ["all", "All"],
          ["starred", "Starred"],
          ["learned", "Learned"],
        ]}
        value={tab}
        onChange={setTab}
      />
      <section className="hero amber">
        <h2>Teach Flow your words</h2>
        <p>Names, product terms and jargon you use. Flow spells them your way every time, and they help it hear them right. It also learns new ones when you correct a word.</p>
        <div className="hero-actions">
          <button className="btn primary dark-solid" onClick={() => open()}>
            Add a new word
          </button>
          {EXAMPLES.map((w) => (
            <button key={w} className="btn glass" onClick={() => open(w)}>
              {w}
            </button>
          ))}
        </div>
      </section>

      {shown.length === 0 ? (
        <Empty title={tab === "all" ? "No words yet" : tab === "starred" ? "No starred words" : "Nothing learned yet"}>
          {tab === "all" ? "Add the names and terms you use often." : tab === "starred" ? "Star a word to give it priority." : "When you fix a word right after pasting, it shows up here."}
        </Empty>
      ) : (
        <div className="list-card">
          {shown.map((e) => (
            <div key={e.id} className="entry">
              <div className="entry-main">
                {e.replacement ? (
                  <>
                    {e.phrase} <span className="arrow">→</span> {e.replacement}
                  </>
                ) : (
                  e.phrase
                )}
                {e.source === "user_edits" && <Sparkles size={15} className="sparkle" aria-label="Learned" />}
              </div>
              {e.uses > 0 && <span className="uses">{e.uses}×</span>}
              {e.starred && <Star size={17} className="star-on" />}
              <div className="entry-tools">
                <button className="icon-btn" title={e.starred ? "Unstar" : "Star (prioritized)"} onClick={() => api.dictSave({ ...e, starred: !e.starred }).then(load)}>
                  <Star />
                </button>
                <button className="icon-btn" title="Delete" onClick={() => api.dictDelete(e.id).then(load)}>
                  <Trash2 />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {settings && (
        <Section>
          <Row title="Learn from my corrections" desc="When you fix a word right after pasting, Flow adds the correct spelling here. You can undo it from the bar.">
            <Toggle on={settings.auto_learn_words} onChange={(v) => save({ auto_learn_words: v })} />
          </Row>
        </Section>
      )}

      {adding && (
        <Modal
          title="Add to dictionary"
          onClose={() => setAdding(null)}
          actions={
            <>
              <button className="btn" onClick={() => setAdding(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={add} disabled={!adding.word.trim()}>
                Add word
              </button>
            </>
          }
        >
          {adding.rule && (
            <label>
              When Flow hears
              <input value={adding.from} placeholder="tool bay" onChange={(e) => setAdding({ ...adding, from: e.target.value })} />
            </label>
          )}
          <label>
            {adding.rule ? "Write it as" : "Word or name"}
            <input autoFocus value={adding.word} placeholder="Toolbay" onChange={(e) => setAdding({ ...adding, word: e.target.value })} onKeyDown={(e) => e.key === "Enter" && add()} />
          </label>
          <label className="check">
            <input type="checkbox" checked={adding.rule} onChange={(e) => setAdding({ ...adding, rule: e.target.checked })} /> Replace a misheard phrase with this spelling
          </label>
        </Modal>
      )}
    </div>
  );
}
