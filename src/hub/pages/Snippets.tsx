import { useEffect, useState } from "react";
import { api, DictEntry } from "../../api";
import { Empty, Icon, Page } from "../ui";

const MAX = 4000;

export default function Snippets() {
  const [list, setList] = useState<DictEntry[]>([]);
  const [edit, setEdit] = useState<Partial<DictEntry> | null>(null);
  const load = () => api.dictList(true).then(setList);
  useEffect(() => {
    load();
  }, []);

  const save = async () => {
    if (!edit?.phrase?.trim() || !edit.replacement?.trim()) return;
    await api.dictSave({ ...edit, is_snippet: true, phrase: edit.phrase.trim() });
    setEdit(null);
    load();
  };

  return (
    <Page
      title="Snippets"
      sub="Say a short cue and Flow types the full text: your email sign-off, calendar link, address or a prompt you reuse."
      actions={
        <button className="btn primary" onClick={() => setEdit({ phrase: "", replacement: "" })}>
          {Icon.plus} New snippet
        </button>
      }
    >
      {edit && (
        <div className="card pad editor">
          <label>
            When I say
            <input autoFocus placeholder="my calendar link" value={edit.phrase ?? ""} onChange={(e) => setEdit({ ...edit, phrase: e.target.value })} />
          </label>
          <label>
            Type this
            <textarea
              rows={5}
              maxLength={MAX}
              placeholder="https://cal.com/you/30min"
              value={edit.replacement ?? ""}
              onChange={(e) => setEdit({ ...edit, replacement: e.target.value })}
            />
            <span className="count">
              {(edit.replacement ?? "").length}/{MAX}
            </span>
          </label>
          <div className="editor-actions">
            <button className="btn" onClick={() => setEdit(null)}>
              Cancel
            </button>
            <button className="btn primary" onClick={save}>
              Save snippet
            </button>
          </div>
        </div>
      )}
      {list.length === 0 && !edit ? (
        <Empty title="No snippets yet">Try one for your email signature: say “my signature” and get the whole block.</Empty>
      ) : (
        <div className="snips">
          {list.map((e) => (
            <div key={e.id} className="card snip" onClick={() => setEdit(e)}>
              <div className="snip-cue">“{e.phrase}”</div>
              <div className="snip-text">{e.replacement}</div>
              <div className="snip-foot">
                {e.uses > 0 ? <span className="uses">used {e.uses}×</span> : <span />}
                <button
                  className="icon-btn"
                  title="Delete"
                  onClick={(ev) => {
                    ev.stopPropagation();
                    api.dictDelete(e.id).then(load);
                  }}
                >
                  {Icon.trash}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </Page>
  );
}
