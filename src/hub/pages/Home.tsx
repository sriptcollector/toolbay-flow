import { useEffect, useMemo, useState } from "react";
import { api, HistoryItem, on, Stats } from "../../api";
import { comboLabel } from "../../keys";
import { dayLabel, Empty, Icon, Page, timeAgo, useSettings } from "../ui";

export default function Home({ go }: { go: (p: string) => void }) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [q, setQ] = useState("");
  const [stats, setStats] = useState<Stats | null>(null);
  const [showRaw, setShowRaw] = useState<number | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [copied, setCopied] = useState<number | null>(null);
  const [settings] = useSettings();

  const load = () => {
    api.historyList(q, 200, 0).then(setItems);
    api.stats().then(setStats);
  };
  useEffect(() => {
    load();
    return on("history-changed", load);
  }, [q]);

  const groups = useMemo(() => {
    const g: [string, HistoryItem[]][] = [];
    for (const it of items) {
      const d = dayLabel(it.created_at);
      if (!g.length || g[g.length - 1][0] !== d) g.push([d, []]);
      g[g.length - 1][1].push(it);
    }
    return g;
  }, [items]);

  const ptt = settings?.shortcuts.find((s) => s.action === "push_to_talk");
  const hf = settings?.shortcuts.find((s) => s.action === "hands_free");

  return (
    <Page
      title="Dictation"
      sub={
        ptt ? (
          <>
            Hold <b>{comboLabel(ptt.keys)}</b> in any app and speak. Release to paste.
            {hf && (
              <>
                {" "}
                Double-tap it, or press <b>{comboLabel(hf.keys)}</b>, for hands-free.
              </>
            )}
          </>
        ) : undefined
      }
    >
      {stats && (
        <div className="stat-strip">
          <div>
            <b>{stats.total_words.toLocaleString()}</b>
            <span>words dictated</span>
          </div>
          <div>
            <b>{Math.round(stats.avg_wpm) || "–"}</b>
            <span>words per minute</span>
          </div>
          <div>
            <b>{stats.streak_days}</b>
            <span>day streak</span>
          </div>
          <div>
            <b>{Math.round(stats.minutes_saved)}</b>
            <span>minutes saved</span>
          </div>
          <button className="link" onClick={() => go("insights")}>
            Insights →
          </button>
        </div>
      )}
      <div className="search">
        <input placeholder="Search your dictations" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      {items.length === 0 ? (
        <Empty title={q ? "Nothing matches that search" : "Your dictations will show up here"}>
          {!q && "Everything you dictate in any app is kept here, on this computer, so you can copy it again or fix it."}
        </Empty>
      ) : (
        groups.map(([day, list]) => (
          <div key={day} className="day">
            <h4>{day}</h4>
            <div className="card list">
              {list.map((it) => (
                <div key={it.id} className="hist">
                  <div className="hist-meta">
                    <span className="time">{timeAgo(it.created_at)}</span>
                    {it.app && <span className="app">{it.app}</span>}
                    {it.mode !== "dictation" && <span className={`tag ${it.mode}`}>{it.mode}</span>}
                  </div>
                  <div className="hist-body">
                    <div className="hist-text">{showRaw === it.id ? it.raw_text : it.text}</div>
                    {showRaw === it.id && <div className="hist-note">What the speech model heard, before cleanup</div>}
                    {it.edited_text && showRaw !== it.id && <div className="hist-note">You edited this after pasting</div>}
                  </div>
                  <div className="hist-actions">
                    <button
                      title="Copy"
                      onClick={() => {
                        api.copyText(showRaw === it.id ? it.raw_text : it.text);
                        setCopied(it.id);
                        setTimeout(() => setCopied(null), 1200);
                      }}
                    >
                      {copied === it.id ? Icon.check : Icon.copy}
                    </button>
                    {it.mode === "dictation" && it.raw_text !== it.text && (
                      <button title="Show original transcript" className={showRaw === it.id ? "on" : ""} onClick={() => setShowRaw(showRaw === it.id ? null : it.id)}>
                        <span className="raw">Aa</span>
                      </button>
                    )}
                    {it.has_audio && (
                      <button
                        title="Transcribe again"
                        disabled={busy === it.id}
                        className={busy === it.id ? "spin" : ""}
                        onClick={async () => {
                          setBusy(it.id);
                          try {
                            await api.historyRetry(it.id);
                          } finally {
                            setBusy(null);
                            load();
                          }
                        }}
                      >
                        {Icon.retry}
                      </button>
                    )}
                    <button title="Delete" onClick={() => api.historyDelete(it.id).then(load)}>
                      {Icon.trash}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))
      )}
    </Page>
  );
}
