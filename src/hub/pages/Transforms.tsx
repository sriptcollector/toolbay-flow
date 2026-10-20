import { Action } from "../../api";
import { Keys, Page, useSettings } from "../ui";

export default function Transforms() {
  const [s, save] = useSettings();
  if (!s) return null;
  const update = (i: number, patch: Partial<{ name: string; instructions: string }>) => {
    const t = s.transforms.map((x, j) => (j === i ? { ...x, ...patch } : x));
    save({ transforms: t });
  };
  return (
    <Page title="Transforms" sub="Highlight text in any app, press a transform shortcut, and Flow rewrites it in place.">
      {s.transforms.map((t, i) => {
        const sc = s.shortcuts.find((x) => x.action === (`transform${i + 1}` as Action));
        return (
          <div key={t.id} className="card pad transform">
            <div className="transform-head">
              <input className="title-input" value={t.name} onChange={(e) => update(i, { name: e.target.value })} />
              {sc && <Keys keys={sc.keys} />}
            </div>
            <textarea rows={3} value={t.instructions} onChange={(e) => update(i, { instructions: e.target.value })} />
          </div>
        );
      })}
      <p className="muted small">
        Transforms use your cleanup model ({s.llm_provider === "local" ? "running on this computer" : "your own API key"}). Command mode can do one-off edits:
        select text, hold the command shortcut and say what you want.
      </p>
    </Page>
  );
}
