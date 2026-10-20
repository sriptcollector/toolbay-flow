import { useEffect, useState } from "react";
import { api, DictEntry, on } from "../../api";
import { Empty, Icon, Page, Row, Section, Toggle, useSettings } from "../ui";

export default function Dictionary() {
  const [list, setList] = useState<DictEntry[]>([]);
  const [word, setWord] = useState("");
  const [rule, setRule] = useState(false);
  const [from, setFrom] = useState("");
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
    if (!word.trim()) return;
    if (rule && from.trim()) await api.dictSave({ phrase: from.trim(), replacement: word.trim() });
    else await api.dictSave({ phrase: word.trim() });
    setWord("");
    setFrom("");
    load();
  };

  return (
    <Page title="Dictionary" sub="Names, product terms and jargon Flow should always spell your way. They also help the speech model hear them.">
      <div className="card pad addbox">
        <div className="addrow">
          {rule && <input placeholder="When I say…" value={from} onChange={(e) => setFrom(e.target.value)} />}
          {rule && <span className="arrow">→</span>}
          <input
            placeholder={rule ? "Write it as…" : "Add a word or name, e.g. Toolbay, Kubernetes, Siobhan"}
            value={word}
            onChange={(e) => setWord(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
          />
          <button className="btn primary" onClick={add}>
            Add
          </button>
        </div>
        <label className="check">
          <input type="checkbox" checked={rule} onChange={(e) => setRule(e.target.checked)} /> Replace a misheard phrase with a different spelling
        </label>
      </div>
      {settings && (
        <Section>
          <Row title="Learn from my corrections" desc="When you fix a word right after pasting, Flow adds the correct spelling here. You can undo it from the bar.">
            <Toggle on={settings.auto_learn_words} onChange={(v) => save({ auto_learn_words: v })} />
          </Row>
        </Section>
      )}
      {list.length === 0 ? (
        <Empty title="No words yet">Add the names and terms you use often.</Empty>
      ) : (
        <div className="card list">
          {list.map((e) => (
            <div key={e.id} className="dict">
              <button
                className={`star ${e.starred ? "on" : ""}`}
                title="Star (prioritized)"
                onClick={() => api.dictSave({ ...e, starred: !e.starred }).then(load)}
              >
                {Icon.star}
              </button>
              <div className="dict-main">
                {e.replacement ? (
                  <>
                    <span className="muted">{e.phrase}</span> <span className="arrow">→</span> <b>{e.replacement}</b>
                  </>
                ) : (
                  <b>{e.phrase}</b>
                )}
              </div>
              {e.source === "user_edits" && <span className="tag learned">Learned</span>}
              {e.uses > 0 && <span className="uses">used {e.uses}×</span>}
              <button className="icon-btn" title="Delete" onClick={() => api.dictDelete(e.id).then(load)}>
                {Icon.trash}
              </button>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}
