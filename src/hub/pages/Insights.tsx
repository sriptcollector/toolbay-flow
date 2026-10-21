import { useEffect, useState } from "react";
import { AppWindow, ChevronLeft, ChevronRight } from "lucide-react";
import { api, on, Stats } from "../../api";
import { Empty } from "../ui";

const TEAL = ["#d4eeed", "#86cfc1", "#3b998c", "#1b5c5c"];
const TYPING_WPM = 40;

/** Something the person has written as much as, in plain words. */
function milestone(words: number) {
  const list: [number, string][] = [
    [100000, "a whole novel"],
    [40000, "a short novel"],
    [7500, "a short story collection"],
    [2500, "a short story"],
    [1000, "a long essay"],
    [250, "a full page"],
  ];
  const hit = list.find(([n]) => words >= n);
  return hit ? `You've written ${hit[1]}!` : "Keep going: your first page is close.";
}

export default function Insights() {
  const [s, setS] = useState<Stats | null>(null);
  const [offset, setOffset] = useState(0);
  useEffect(() => {
    const load = () => api.stats().then(setS);
    load();
    return on("history-changed", load);
  }, []);
  if (!s) return null;

  const wpm = Math.round(s.avg_wpm) || 0;
  const ratio = wpm / TYPING_WPM;
  const gauge = Math.min(1, wpm / 200);

  // Heatmap: 13 weeks, Sunday-first rows, newest column on the right.
  const byDay = new Map(s.daily);
  const weeks = 13;
  const today = new Date();
  const end = new Date(today);
  end.setDate(today.getDate() - offset * weeks * 7 + (6 - today.getDay()));
  const start = new Date(end);
  start.setDate(end.getDate() - (weeks * 7 - 1));
  const cells: { d: Date; key: string; v: number }[] = [];
  for (let i = 0; i < weeks * 7; i++) {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    cells.push({ d, key, v: d > today ? -1 : byDay.get(key) ?? 0 });
  }
  const max = Math.max(1, ...cells.map((c) => c.v));
  const level = (v: number) => (v <= 0 ? -1 : Math.min(3, Math.floor((4 * v) / (max + 1))));
  const months: string[] = [];
  for (let w = 0; w < weeks; w++) {
    const d = cells[w * 7].d;
    const label = d.toLocaleDateString([], { month: "short" });
    months.push(w === 0 || cells[(w - 1) * 7].d.getMonth() !== d.getMonth() ? label : "");
  }
  const streakStart = new Date(today);
  streakStart.setDate(today.getDate() - Math.max(0, s.streak_days - 1));
  const total = s.top_apps.reduce((a, [, w]) => a + w, 0) || 1;

  return (
    <div className="page insights">
      <header className="page-head">
        <h1>Insights</h1>
      </header>

      <div className="ins-top">
        <div className="ins-card">
          <b className="ins-n">{wpm}</b>
          <span className="ins-l">Words per minute</span>
          <div className="gauge">
            <svg viewBox="0 0 200 110">
              <path d="M20 100 A80 80 0 0 1 180 100" stroke="var(--sand-700)" strokeWidth="18" fill="none" strokeLinecap="round" />
              <path d="M20 100 A80 80 0 0 1 180 100" stroke="#1b5c5c" strokeWidth="18" fill="none" strokeLinecap="round" pathLength={100} strokeDasharray={`${gauge * 100} 100`} />
            </svg>
            <div className="gauge-text">
              <span>vs typing</span>
              <b>{ratio ? `${ratio.toFixed(1)}×` : "–"}</b>
            </div>
          </div>
        </div>
        <div className="ins-card">
          <b className="ins-n">{formatMinutes(s.minutes_saved)}</b>
          <span className="ins-l">Time saved</span>
          <hr />
          <p>{s.total_dictations.toLocaleString()} dictations</p>
          <p>{s.words_this_week.toLocaleString()} words this week</p>
        </div>
        <div className="ins-card">
          <b className="ins-n">{s.total_words.toLocaleString()}</b>
          <span className="ins-l">Total words dictated</span>
          <hr />
          <p>{milestone(s.total_words)}</p>
        </div>
      </div>

      <div className="ins-bottom">
        <div className="ins-card">
          <div className="ins-head">
            <h2>App usage</h2>
            <span className="ins-l">Total apps used | {s.top_apps.length}</span>
          </div>
          {s.top_apps.length === 0 ? (
            <Empty title="No apps yet" />
          ) : (
            s.top_apps.slice(0, 6).map(([app, w], i) => {
              const pct = Math.round((100 * w) / total);
              return (
                <div key={app} className="use-row">
                  <AppWindow size={22} strokeWidth={1.5} />
                  <span className="use-bar" style={{ width: `${Math.max(12, pct)}%`, background: TEAL[3 - Math.min(2, Math.floor(i / 2))] }}>
                    {pct}%
                  </span>
                  <span className="use-l">
                    {w.toLocaleString()} words in {app}
                  </span>
                </div>
              );
            })
          )}
        </div>
        <div className="ins-card">
          <div className="ins-head">
            <h2>
              {s.streak_days} day streak
            </h2>
          </div>
          <div className="heatmap">
            <div className="hm-months">
              <button className="icon-btn" aria-label="Earlier" onClick={() => setOffset(offset + 1)}>
                <ChevronLeft />
              </button>
              <div className="hm-month-row" style={{ gridTemplateColumns: `repeat(${weeks}, 1fr)` }}>
                {months.map((m, i) => (
                  <span key={i}>{m}</span>
                ))}
              </div>
              <button className="icon-btn" aria-label="Later" disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - 1))}>
                <ChevronRight />
              </button>
            </div>
            <div className="hm-grid">
              {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map((day, r) => (
                <div key={day} className="hm-row" style={{ gridTemplateColumns: `36px repeat(${weeks}, 1fr)` }}>
                  <span className="hm-day">{day}</span>
                  {Array.from({ length: weeks }).map((_, w) => {
                    const c = cells[w * 7 + r];
                    const l = level(c.v);
                    const inStreak = s.streak_days > 0 && c.d >= streakStart && c.d <= today;
                    return (
                      <i
                        key={c.key}
                        title={c.v >= 0 ? `${c.key}: ${c.v} words` : ""}
                        className={inStreak ? "streak" : ""}
                        style={{ visibility: c.v < 0 ? "hidden" : "visible", background: l < 0 ? "var(--fill)" : TEAL[l] }}
                      />
                    );
                  })}
                </div>
              ))}
            </div>
            <div className="hm-legend">
              <span>More</span>
              {[...TEAL].reverse().map((c) => (
                <i key={c} style={{ background: c }} />
              ))}
              <span>Less</span>
              <span className="hm-fill" />
              <i className="streak-key" />
              <span>Current streak</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function formatMinutes(m: number) {
  if (m < 60) return `${Math.round(m)} min`;
  return `${(m / 60).toFixed(1)} h`;
}
