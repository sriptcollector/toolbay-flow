import { ReactNode, useEffect, useState } from "react";
import { api, on } from "../api";
import { Icon, useSettings } from "./ui";
import Onboarding from "./Onboarding";
import Home from "./pages/Home";
import Insights from "./pages/Insights";
import Dictionary from "./pages/Dictionary";
import Snippets from "./pages/Snippets";
import StylePage from "./pages/Style";
import Transforms from "./pages/Transforms";
import Notes from "./pages/Notes";
import Models from "./pages/Models";
import Shortcuts from "./pages/Shortcuts";
import SettingsPage from "./pages/Settings";

type PageId = "home" | "insights" | "dictionary" | "snippets" | "style" | "transforms" | "notes" | "models" | "shortcuts" | "settings";

const NAV: [PageId, string, ReactNode][] = [
  ["home", "Dictation", Icon.home],
  ["insights", "Insights", Icon.chart],
  ["dictionary", "Dictionary", Icon.book],
  ["snippets", "Snippets", Icon.bolt],
  ["style", "Style", Icon.brush],
  ["transforms", "Transforms", Icon.wand],
  ["notes", "Scratchpad", Icon.note],
];
const NAV2: [PageId, string, ReactNode][] = [
  ["models", "Models", Icon.chip],
  ["shortcuts", "Shortcuts", Icon.keyboard],
  ["settings", "Settings", Icon.gear],
];

export default function Hub() {
  const [settings, , reload] = useSettings();
  const [page, setPage] = useState<PageId>("home");
  const [version, setVersion] = useState("");

  useEffect(() => {
    api.appInfo().then((i) => setVersion(i.version));
    return on<string>("navigate", (p) => setPage(p as PageId));
  }, []);

  if (!settings) return <div className="boot" />;
  if (!settings.onboarded) return <Onboarding onDone={reload} />;

  const Current = {
    home: Home,
    insights: Insights,
    dictionary: Dictionary,
    snippets: Snippets,
    style: StylePage,
    transforms: Transforms,
    notes: Notes,
    models: Models,
    shortcuts: Shortcuts,
    settings: SettingsPage,
  }[page];

  return (
    <div className="hub">
      <aside className="side">
        <div className="brand">
          <Logo />
          <div>
            <div className="brand-name">Flow</div>
            <div className="brand-by">by Toolbay</div>
          </div>
        </div>
        <nav>
          {NAV.map(([id, label, icon]) => (
            <button key={id} className={page === id ? "active" : ""} onClick={() => setPage(id)}>
              {icon}
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="side-spacer" />
        <nav>
          {NAV2.map(([id, label, icon]) => (
            <button key={id} className={page === id ? "active" : ""} onClick={() => setPage(id)}>
              {icon}
              <span>{label}</span>
            </button>
          ))}
        </nav>
        <div className="side-foot">
          <span className="free-pill">Free · Local · Open source</span>
          <span className="ver">v{version}</span>
        </div>
      </aside>
      <main className="content">
        <Current key={page} go={(p: string) => setPage(p as PageId)} />
      </main>
    </div>
  );
}

export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className="logo">
      <rect width="32" height="32" rx="9" fill="#141414" stroke="#2f2f2f" />
      <g fill="#fff">
        <rect x="7" y="13" width="3" height="6" rx="1.5" />
        <rect x="12" y="9" width="3" height="14" rx="1.5" />
        <rect x="17" y="11" width="3" height="10" rx="1.5" />
        <rect x="22" y="14" width="3" height="4" rx="1.5" fill="#8b6dff" />
      </g>
    </svg>
  );
}
