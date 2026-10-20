import { useState } from "react";
import { Settings, Style } from "../../api";
import { Page, Section, useSettings } from "../ui";

const CATS: [keyof Settings["styles"], string, string, string][] = [
  ["personal", "Personal messages", "WhatsApp, Messages, Discord, Telegram", "hey are we still on for tonight i can bring snacks"],
  ["work", "Work messages", "Slack, Teams, Linear, Notion", "Quick update: the build is green and I'll ship it after lunch"],
  ["email", "Email", "Gmail, Outlook and other mail apps", "Hi Dana, thanks for the notes. I'll send the revised draft by Friday. Best, Sam"],
  ["other", "Everything else", "Docs, browsers, editors, AI chats", "The results suggest a clear improvement in retention over the last quarter."],
];

const STYLES: [Style, string, string][] = [
  ["formal", "Formal", "Capitals and full punctuation"],
  ["casual", "Casual", "Capitals, lighter punctuation"],
  ["very_casual", "Very casual", "lowercase, minimal punctuation"],
  ["excited", "Excited", "Upbeat, more exclamation marks!"],
];

function sample(text: string, s: Style) {
  let t = text;
  if (s === "casual") t = t.replace(/\.$/, "");
  if (s === "very_casual") t = t.charAt(0).toLowerCase() + t.slice(1).replace(/[.,]/g, "").replace(/:/g, "");
  if (s === "excited") t = t.replace(/\.$/, "!").replace(/(tonight)/, "$1!");
  if (s === "formal") {
    t = t.charAt(0).toUpperCase() + t.slice(1);
    if (!/[.!?]$/.test(t)) t += ".";
    t = t.replace(/^hey are/, "Hey, are").replace(/tonight i/, "tonight? I");
  }
  return t;
}

export default function StylePage() {
  const [s, save] = useSettings();
  const [app, setApp] = useState("");
  const [cat, setCat] = useState("work");
  if (!s) return null;
  return (
    <Page title="Style" sub="Flow writes differently depending on where you are. Pick how each kind of app should sound.">
      {CATS.map(([key, title, apps, text]) => (
        <section key={key} className="section">
          <div className="section-head">
            <h3>{title}</h3>
            <span className="muted small">{apps}</span>
          </div>
          <div className="styles">
            {STYLES.map(([id, label, desc]) => (
              <button
                key={id}
                className={`card style-card ${s.styles[key] === id ? "sel" : ""}`}
                onClick={() => save({ styles: { ...s.styles, [key]: id } })}
              >
                <div className="style-label">{label}</div>
                <div className="style-desc">{desc}</div>
                <div className="style-sample">{sample(text, id)}</div>
              </button>
            ))}
          </div>
        </section>
      ))}
      <Section title="Your own app rules" aside={<span className="muted small">Use the program name, e.g. notepad.exe</span>}>
        <div className="pad">
          {Object.entries(s.app_categories).map(([k, v]) => (
            <div key={k} className="maprow">
              <code>{k}</code>
              <span className="arrow">→</span>
              <span>{CATS.find((c) => c[0] === v)?.[1] ?? v}</span>
              <button
                className="link"
                onClick={() => {
                  const next = { ...s.app_categories };
                  delete next[k];
                  save({ app_categories: next });
                }}
              >
                Remove
              </button>
            </div>
          ))}
          <div className="addrow">
            <input placeholder="app.exe" value={app} onChange={(e) => setApp(e.target.value)} />
            <select value={cat} onChange={(e) => setCat(e.target.value)}>
              {CATS.map((c) => (
                <option key={c[0]} value={c[0]}>
                  {c[1]}
                </option>
              ))}
            </select>
            <button
              className="btn"
              onClick={() => {
                if (!app.trim()) return;
                save({ app_categories: { ...s.app_categories, [app.trim().toLowerCase()]: cat } });
                setApp("");
              }}
            >
              Add rule
            </button>
          </div>
        </div>
      </Section>
    </Page>
  );
}
