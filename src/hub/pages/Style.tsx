import { useState } from "react";
import { Settings, Style } from "../../api";
import { Section, Tabs, useSettings } from "../ui";

type Cat = keyof Settings["styles"];
const CATS: [Cat, string, string, string, string][] = [
  ["personal", "Personal messages", "This style is used in personal messages", "WhatsApp, Messages, Discord, Telegram", "hey are you free for dinner tomorrow? let's do 7 if that works for you"],
  ["work", "Work messages", "This style is used in work chats", "Slack, Teams, Linear, Notion", "Quick update: the build is green and I'll ship it after lunch"],
  ["email", "Email", "This style is used in email", "Gmail, Outlook and other mail apps", "Hi Dana, thanks for the notes. I'll send the revised draft by Friday."],
  ["other", "Other", "This style is used everywhere else", "Docs, browsers, editors, AI chats", "The results suggest a clear improvement in retention over the last quarter."],
];

const STYLES: [Style, string, string][] = [
  ["formal", "Formal.", "Caps + Punctuation"],
  ["casual", "Casual", "Caps + Less punctuation"],
  ["very_casual", "very casual", "No caps + Less punctuation"],
  ["excited", "Excited!", "Caps + Exclamation points"],
];

function sample(text: string, s: Style) {
  let t = text.charAt(0).toUpperCase() + text.slice(1);
  t = t.replace(/^Hey are/, "Hey, are").replace(/tomorrow\? let's/, "tomorrow? Let's");
  if (!/[.!?]$/.test(t)) t += ".";
  if (s === "casual") t = t.replace(/\.$/, "");
  if (s === "very_casual") t = t.charAt(0).toLowerCase() + t.slice(1).replace(/[.,:]/g, "").replace(/\? L/, "? l");
  if (s === "excited") t = t.replace(/\.$/, "!");
  return t;
}

const AVATAR: Record<Style, string> = { formal: "#ecd5fb", casual: "#ffc4f0", very_casual: "#7232a6", excited: "#ffa946" };

export default function StylePage() {
  const [s, save] = useSettings();
  const [tab, setTab] = useState<Cat>("personal");
  const [app, setApp] = useState("");
  if (!s) return null;
  const cat = CATS.find((c) => c[0] === tab)!;
  const initial = (s.name.trim()[0] || "Y").toUpperCase();
  return (
    <div className="page">
      <header className="page-head">
        <h1>Styles</h1>
      </header>
      <Tabs tabs={CATS.map((c) => [c[0], c[1]] as [Cat, string])} value={tab} onChange={setTab} />
      <section className="hero dusk hero-row">
        <div>
          <h2 className="hero-h-sm">{cat[2]}</h2>
          <p>
            {cat[3]}. Style formatting applies to English for now.
          </p>
        </div>
      </section>
      <div className="style-cards">
        {STYLES.map(([id, label, desc]) => (
          <button key={id} className={`style-card ${s.styles[tab] === id ? "sel" : ""}`} onClick={() => save({ styles: { ...s.styles, [tab]: id } })}>
            <div className="style-label serif">{label}</div>
            <div className="style-desc">{desc}</div>
            <div className="bubble">{sample(cat[4], id)}</div>
            <div className="avatar" style={{ background: AVATAR[id], color: id === "very_casual" ? "#fff" : "#fff" }}>
              {initial}
            </div>
          </button>
        ))}
      </div>

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
            <span className="muted">uses the {cat[1].toLowerCase()} style</span>
            <button
              className="btn"
              onClick={() => {
                if (!app.trim()) return;
                save({ app_categories: { ...s.app_categories, [app.trim().toLowerCase()]: tab } });
                setApp("");
              }}
            >
              Add rule
            </button>
          </div>
        </div>
      </Section>
    </div>
  );
}
