import { Action } from "../../api";
import { Keys, Page, SettingText, useSettings } from "../ui";

export default function Transforms() {
  const [s, save] = useSettings();
  if (!s) return null;
  // Applied to the latest settings, so an edit to one field never undoes another.
  const update = (i: number, patch: Partial<{ name: string; instructions: string }>) =>
    save((cur) => ({ transforms: cur.transforms.map((x, j) => (j === i ? { ...x, ...patch } : x)) }));
  return (
    <Page title="Transforms" sub="Highlight text in any app, press a transform shortcut, and Toolbay Flow rewrites it in place.">
      {s.transforms.map((t, i) => {
        const sc = s.shortcuts.find((x) => x.action === (`transform${i + 1}` as Action));
        return (
          <div key={t.id} className="card pad transform">
            <div className="transform-head">
              <SettingText className="title-input" value={t.name} onSave={(v) => update(i, { name: v })} />
              {sc && <Keys keys={sc.keys} />}
            </div>
            <SettingText multiline rows={3} value={t.instructions} onSave={(v) => update(i, { instructions: v })} />
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
