import { useEffect, useMemo, useState } from "react";
import { Hourglass, Search, X } from "lucide-react";
import { api, HistoryItem, on, Stats } from "../../api";
import { comboLabel } from "../../keys";
import { dayLabel, Empty, Icon, timeAgo, useSettings } from "../ui";

export default function Home({ go }: { go: (p: string) => void }) {
  const [items, setItems] = useState<HistoryItem[]>([]);
  const [q, setQ] = useState("");
  const [stats, setStats] = useState<Stats | null>(null);
  const [showRaw, setShowRaw] = useState<number | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [copied, setCopied] = useState<number | null>(null);
  const [hideHero, setHideHero] = useState(() => localStorage.getItem("flow.hero.home") === "off");
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
  const first = settings?.name?.trim().split(/\s+/)[0];

  return (
    <div className="page home">
      <div className="home-grid">
        <div className="home-main">
          <header className="home-head">
            <h1>{first ? `Welcome back, ${first}` : "Welcome back"}</h1>
            <label className="home-search">
              <Search size={17} strokeWidth={1.6} />
              <input placeholder="Search" value={q} onChange={(e) => setQ(e.target.value)} />
            </label>
          </header>
          {!hideHero && ptt && (
            <section className="hero dusk">
              <button
                className="hero-close"
                aria-label="Close"
                onClick={() => {
                  localStorage.setItem("flow.hero.home", "off");
                  setHideHero(true);
                }}
              >
                <X />
              </button>
              <h2>Talk instead of typing</h2>
              <p>
                Hold <b>{comboLabel(ptt.keys)}</b> in any app, say what you want to write, and let go.
                {hf && (
                  <>
                    {" "}
                    For longer thoughts, press <b>{comboLabel(hf.keys)}</b> and talk hands-free.
                  </>
                )}
              </p>
              <div className="hero-actions">
                <button className="btn on-dark" onClick={() => go("shortcuts")}>
                  See my shortcuts
                </button>
              </div>
            </section>
          )}
          {items.length === 0 ? (
            <Empty title={q ? "Nothing matches that search" : "Your dictations will show up here"}>
              {!q && "Everything you dictate in any app is kept here, on this computer, so you can copy it again or fix it."}
            </Empty>
          ) : (
            groups.map(([day, list]) => (
              <div key={day} className="day">
                <h4 className="label">{day}</h4>
                <div className="list-card">
                  {list.map((it) => (
                    <div key={it.id} className="hist">
                      <div className="hist-meta">
                        <span className="time">{timeAgo(it.created_at)}</span>
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
        </div>
        {stats && (
          <aside className="home-side">
            <div className="stat-card">
              <button className="stat-top" onClick={() => go("insights")}>
                <div>
                  <b className="serif">{stats.total_words.toLocaleString()}</b> total words
                </div>
                <div>
                  <b className="serif">{Math.round(stats.avg_wpm) || 0}</b> wpm
                </div>
                <div>
                  <b className="serif">{stats.streak_days}</b> day streak
                </div>
              </button>
              <div className="stat-bottom">
                <div>
                  <div className="stat-title">Time saved</div>
                  <div className="stat-sub">{Math.round(stats.minutes_saved)} minutes you didn't spend typing</div>
                </div>
                <Hourglass size={44} strokeWidth={1.2} />
              </div>
            </div>
          </aside>
        )}
      </div>
    </div>
  );
}
