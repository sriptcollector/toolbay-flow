import { ReactNode, useEffect, useState } from "react";
import { MessageSquareHeart, Mic, ChartColumnBig, BookA, Scissors, Type, WandSparkles, NotebookPen, Cpu, Keyboard, Settings as Gear, CircleHelp, PanelLeft, CircleUserRound, Minus, Square, X } from "lucide-react";
import { api, on } from "../api";
import { Toaster, useSettings } from "./ui";
import Onboarding from "./Onboarding";
import Feedback from "./Feedback";
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

const I = { size: 20, strokeWidth: 1.6 };
const NAV: [PageId, string, ReactNode][] = [
  ["home", "Dictation", <Mic {...I} />],
  ["insights", "Insights", <ChartColumnBig {...I} />],
  ["dictionary", "Dictionary", <BookA {...I} />],
  ["snippets", "Snippets", <Scissors {...I} />],
  ["style", "Style", <Type {...I} />],
  ["transforms", "Transforms", <WandSparkles {...I} />],
  ["notes", "Scratchpad", <NotebookPen {...I} />],
];
const NAV2: [PageId, string, ReactNode][] = [
  ["models", "Models", <Cpu {...I} />],
  ["shortcuts", "Shortcuts", <Keyboard {...I} />],
  ["settings", "Settings", <Gear {...I} />],
];

/** Applies the Appearance setting (light by default, like the original). */
export function useTheme(appearance?: string) {
  useEffect(() => {
    const apply = () => {
      const dark = appearance === "dark" || (appearance === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
    };
    apply();
    const mq = matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [appearance]);
}

async function win() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

export function TitleBar({ children }: { children?: ReactNode }) {
  const [os, setOs] = useState("");
  useEffect(() => {
    api.appInfo().then((i) => setOs(i.os));
  }, []);
  return (
    <header className="titlebar" data-tauri-drag-region>
      {children}
      <div className="tb-fill" data-tauri-drag-region />
      {os === "windows" && (
        <div className="caption">
          <button aria-label="Minimize" onClick={() => win().then((w) => w.minimize())}>
            <Minus />
          </button>
          <button aria-label="Maximize" onClick={() => win().then((w) => w.toggleMaximize())}>
            <Square size={14} />
          </button>
          <button aria-label="Close" className="close" onClick={() => win().then((w) => w.close())}>
            <X />
          </button>
        </div>
      )}
    </header>
  );
}

export default function Hub() {
  const [settings, , reload] = useSettings();
  // ?page=<id> opens a page directly (used for screenshots and the comparison sheet).
  const start = new URLSearchParams(location.search).get("page");
  const [page, setPage] = useState<PageId>(() => (start && start !== "feedback" ? (start as PageId) : "home"));
  const [collapsed, setCollapsed] = useState(false);
  const [mac, setMac] = useState(false);
  const [feedback, setFeedback] = useState(start === "feedback");
  useTheme(settings?.appearance);

  useEffect(() => {
    api.appInfo().then((i) => setMac(i.os === "macos"));
    const offFb = on("open-feedback", () => setFeedback(true));
    const offNav = on<string>("navigate", (p) => (p === "feedback" ? setFeedback(true) : setPage(p as PageId)));
    return () => {
      offFb();
      offNav();
    };
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

  const item = ([id, label, icon]: [PageId, string, ReactNode]) => (
    <button key={id} className={page === id ? "active" : ""} onClick={() => setPage(id)} title={collapsed ? label : undefined}>
      {icon}
      <span>{label}</span>
    </button>
  );

  return (
    <div className={`hub ${collapsed ? "collapsed" : ""} ${mac ? "mac" : ""}`}>
      <TitleBar>
        <button className="tb-btn" aria-label="Toggle sidebar" onClick={() => setCollapsed(!collapsed)}>
          <PanelLeft />
        </button>
        <button className="tb-btn" aria-label="Toolbay account" onClick={() => setPage("settings")}>
          <CircleUserRound />
        </button>
      </TitleBar>
      <aside className="side">
        <div className="brand">
          <Logo size={28} />
          <span className="brand-name">Toolbay Flow</span>
        </div>
        <nav>{NAV.map(item)}</nav>
        <div className="side-spacer" />
        <nav>
          {NAV2.map(item)}
          <button onClick={() => setFeedback(true)} title={collapsed ? "Share feedback" : undefined}>
            <MessageSquareHeart {...I} />
            <span>Share feedback</span>
          </button>
          <a href="https://github.com/sriptcollector/toolbay-flow#readme" target="_blank" rel="noreferrer" title={collapsed ? "Help" : undefined}>
            <CircleHelp {...I} />
            <span>Help</span>
          </a>
        </nav>
      </aside>
      <main className="content">
        <Current key={page} go={(p: string) => (p === "feedback" ? setFeedback(true) : setPage(p as PageId))} />
      </main>
      {feedback && <Feedback onClose={() => setFeedback(false)} />}
      <Toaster />
    </div>
  );
}

/** Toolbay Flow's own mark: four rounded voice bars in a soft square. */
export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" className="logo" style={{ stroke: "none" }}>
      <rect width="32" height="32" rx="9" fill="var(--ink)" />
      <g fill="var(--surface)">
        <rect x="7" y="13" width="3" height="6" rx="1.5" />
        <rect x="12" y="9" width="3" height="14" rx="1.5" />
        <rect x="17" y="11" width="3" height="10" rx="1.5" />
        <rect x="22" y="14" width="3" height="4" rx="1.5" fill="var(--lavender-2)" />
      </g>
    </svg>
  );
}
