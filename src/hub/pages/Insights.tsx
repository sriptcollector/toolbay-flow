import { useEffect, useState } from "react";
import { api, on, Stats } from "../../api";
import { Empty, Page } from "../ui";

export default function Insights() {
  const [s, setS] = useState<Stats | null>(null);
  useEffect(() => {
    const load = () => api.stats().then(setS);
    load();
    return on("history-changed", load);
  }, []);
  if (!s) return null;

  const byDay = new Map(s.daily);
  const weeks = 18;
  const today = new Date();
  const start = new Date(today);
  start.setDate(today.getDate() - (weeks * 7 - 1) - today.getDay());
  const cells: { d: string; v: number }[] = [];
  for (let i = 0; i < weeks * 7; i++) {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    cells.push({ d: key, v: d > today ? -1 : byDay.get(key) ?? 0 });
  }
  const max = Math.max(1, ...cells.map((c) => c.v));
  const topMax = Math.max(1, ...s.top_apps.map((a) => a[1]));

  return (
    <Page title="Insights" sub="How much you talk instead of type. All counted on this computer.">
      <div className="tiles">
        <Tile n={s.total_words.toLocaleString()} l="Total words" />
        <Tile n={s.words_this_week.toLocaleString()} l="Words this week" />
        <Tile n={Math.round(s.avg_wpm) || "–"} l="Average words per minute" hint="Typing averages about 40" />
        <Tile n={`${s.streak_days} ${s.streak_days === 1 ? "day" : "days"}`} l="Current streak" />
        <Tile n={formatMinutes(s.minutes_saved)} l="Time saved vs typing" />
        <Tile n={s.total_dictations.toLocaleString()} l="Dictations" />
      </div>
      <section className="section">
        <div className="section-head">
          <h3>Activity</h3>
        </div>
        <div className="card pad">
          <div className="heat" style={{ gridTemplateColumns: `repeat(${weeks}, 1fr)` }}>
            {Array.from({ length: weeks }).map((_, w) => (
              <div key={w} className="heat-col">
                {cells.slice(w * 7, w * 7 + 7).map((c) => (
                  <i
                    key={c.d}
                    title={c.v >= 0 ? `${c.d}: ${c.v} words` : ""}
                    style={{ opacity: c.v < 0 ? 0 : 1, background: c.v > 0 ? `rgba(139, 109, 255, ${0.2 + (0.8 * c.v) / max})` : undefined }}
                  />
                ))}
              </div>
            ))}
          </div>
        </div>
      </section>
      <section className="section">
        <div className="section-head">
          <h3>Where you dictate</h3>
        </div>
        <div className="card pad">
          {s.top_apps.length === 0 ? (
            <Empty title="No apps yet" />
          ) : (
            s.top_apps.map(([app, w]) => (
              <div key={app} className="barrow">
                <span className="barrow-l">{app}</span>
                <span className="barrow-b">
                  <i style={{ width: `${(100 * w) / topMax}%` }} />
                </span>
                <span className="barrow-n">{w.toLocaleString()}</span>
              </div>
            ))
          )}
        </div>
      </section>
    </Page>
  );
}

function Tile({ n, l, hint }: { n: string | number; l: string; hint?: string }) {
  return (
    <div className="tile">
      <b>{n}</b>
      <span>{l}</span>
      {hint && <small>{hint}</small>}
    </div>
  );
}

function formatMinutes(m: number) {
  if (m < 60) return `${Math.round(m)} min`;
  return `${(m / 60).toFixed(1)} h`;
}
