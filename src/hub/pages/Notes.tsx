import { useCallback, useEffect, useRef, useState } from "react";
import { api, Note } from "../../api";
import { Icon, Page, registerFlush } from "../ui";

export default function Notes() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [cur, setCur] = useState<Note | null>(null);
  const timer = useRef<number | undefined>(undefined);
  // The last edit, until it's saved (400 ms after typing stops, on leaving the
  // note or the page, and before Flow quits).
  const pending = useRef<Note | null>(null);
  const ta = useRef<HTMLTextAreaElement>(null);

  const flush = useCallback(() => {
    window.clearTimeout(timer.current);
    const n = pending.current;
    pending.current = null;
    return n ? api.noteSave(n) : Promise.resolve();
  }, []);
  useEffect(() => {
    const off = registerFlush(flush);
    return () => {
      off();
      flush();
    };
  }, [flush]);

  const load = (select?: number) =>
    api.notesList().then((n) => {
      setNotes(n);
      if (select !== undefined) setCur(n.find((x) => x.id === select) ?? null);
      else if (!cur && n.length) setCur(n[0]);
    });
  useEffect(() => {
    load();
  }, []);

  const create = async () => {
    const id = await api.noteSave({ content: "" });
    await load(id);
    setTimeout(() => ta.current?.focus(), 30);
  };

  const change = (content: string) => {
    if (!cur) return;
    const n = { ...cur, content };
    setCur(n);
    setNotes((all) => all.map((x) => (x.id === n.id ? n : x)));
    pending.current = n;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(flush, 400);
  };

  return (
    <Page
      title="Scratchpad"
      sub="A quick place for thoughts. Click into a note and dictate."
      actions={
        <button className="btn primary" onClick={create}>
          {Icon.plus} New note
        </button>
      }
    >
      <div className="notes">
        <div className="card notes-list">
          {notes.length === 0 && <div className="muted pad">No notes yet</div>}
          {notes.map((n) => (
            <button key={n.id} className={`note-item ${cur?.id === n.id ? "sel" : ""}`} onClick={() => (flush(), setCur(n))}>
              <div className="note-title">
                {n.pinned && "📌 "}
                {n.content.split("\n")[0].slice(0, 60) || "Untitled"}
              </div>
              <div className="note-date">{new Date(n.updated_at).toLocaleString([], { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}</div>
            </button>
          ))}
        </div>
        <div className="card notes-editor">
          {cur ? (
            <>
              <div className="notes-tools">
                <button className="link" onClick={() => flush().then(() => api.noteSave({ ...cur, pinned: !cur.pinned })).then(() => load(cur.id))}>
                  {cur.pinned ? "Unpin" : "Pin"}
                </button>
                <button className="link" onClick={() => api.copyText(cur.content)}>
                  Copy
                </button>
                <button
                  className="link danger"
                  onClick={() => {
                    // Drop the unsaved edit: saving it would bring the note back.
                    window.clearTimeout(timer.current);
                    pending.current = null;
                    api.noteDelete(cur.id).then(() => {
                      setCur(null);
                      load();
                    });
                  }}
                >
                  Delete
                </button>
              </div>
              <textarea ref={ta} value={cur.content} placeholder="Hold your dictation shortcut and start talking…" onChange={(e) => change(e.target.value)} />
            </>
          ) : (
            <div className="muted pad">Create a note to start.</div>
          )}
        </div>
      </div>
    </Page>
  );
}
