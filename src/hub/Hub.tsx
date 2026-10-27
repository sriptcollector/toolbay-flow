import { ReactNode, useEffect, useState } from "react";
import { MessageSquareHeart, Mic, ChartColumnBig, BookA, Scissors, Type, WandSparkles, NotebookPen, Cpu, Keyboard, Settings as Gear, CircleHelp, PanelLeft, CircleUserRound, Minus, Square, X } from "lucide-react";
import { api, on } from "../api";
import { Toaster, useSettings } from "./ui";
import Onboarding from "./Onboarding";
import { askedThisRun, clearPermissionResume, markPermissionAsked, resumedFrom } from "./permResume";
import Feedback from "./Feedback";
import logoPixel from "../assets/logo-pixel.png";
import logoPixelLight from "../assets/logo-pixel-light.png";
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
      // The Dock / taskbar icon follows too.
      import("@tauri-apps/api/core").then(({ invoke }) => invoke("set_app_icon", { dark })).catch(() => {});
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
        {mac && <MacPermissionAlert />}
        <Current key={page} go={(p: string) => (p === "feedback" ? setFeedback(true) : setPage(p as PageId))} />
      </main>
      {feedback && <Feedback onClose={() => setFeedback(false)} />}
      <Toaster />
    </div>
  );
}

/** Shown on every page until macOS allows typing and the shortcut; without them dictation can't start. */
function MacPermissionAlert() {
  const [settings] = useSettings();
  const [p, setP] = useState<{ accessibility: boolean; input: boolean; fn_free: boolean } | null>(null);
  const [resumed] = useState(() => resumedFrom() === "app");
  const [asked, setAsked] = useState(askedThisRun);
  const [thanks, setThanks] = useState(false);
  useEffect(() => {
    const check = () => api.macPermissions().then(setP).catch(() => {});
    check();
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, []);
  const ok = !!p && p.accessibility && p.input;
  useEffect(() => {
    if (!resumed || !ok) return;
    clearPermissionResume();
    setThanks(true);
    const t = setTimeout(() => setThanks(false), 8000);
    return () => clearTimeout(t);
  }, [resumed, ok]);
  if (!p) return null;
  const allow = (kind: string) => {
    markPermissionAsked("app");
    setAsked(true);
    api.openSystemSettings(kind);
  };
  const usesFn = settings?.shortcuts.some((s) => s.keys.includes(63)) ?? false;
  if (ok && thanks) {
    return (
      <div className="perm-result ok" role="status">
        Restarted. Toolbay Flow can now hear your shortcut and type for you.
      </div>
    );
  }
  if (ok) {
    if (!usesFn || p.fn_free) return null;
    return (
      <div className="perm-alert" role="alert">
        <div>
          <b>macOS is also using the Fn key</b>
          <span>In Keyboard settings, set “Press 🌐 key to” to “Do Nothing”, so holding Fn starts Toolbay Flow instead of emoji or dictation.</span>
        </div>
        <button className="btn" onClick={() => api.openSystemSettings("keyboard")}>
          Open Keyboard settings
        </button>
      </div>
    );
  }
  return (
    <div className="perm-alert" role="alert">
      <div>
        <b>Your shortcut won't work until macOS allows Toolbay Flow</b>
        <span>
          {resumed
            ? "Restarted, but macOS still has one off. Switch Toolbay Flow on for it, then restart again."
            : asked
              ? "Switched it on? macOS applies it after Toolbay Flow restarts. You'll come right back here."
              : "Click each button, then switch Toolbay Flow on in the window that opens. If it already looks on, Flow clears the old entry so you can switch it on fresh."}
        </span>
      </div>
      {(asked || resumed) && (
        <button className="btn" onClick={() => api.restartApp()}>
          Restart Toolbay Flow
        </button>
      )}
      {!p.accessibility && (
        <button className="btn" onClick={() => allow("accessibility")}>
          Allow Accessibility
        </button>
      )}
      {!p.input && (
        <button className="btn" onClick={() => allow("input")}>
          Allow Input Monitoring
        </button>
      )}
    </div>
  );
}

/** Toolbay Flow's own mark: the app icon, pixel art like the rest of Toolbay's apps. */
export function Logo({ size = 30 }: { size?: number }) {
  const isDark = () => document.documentElement.dataset.theme === "dark";
  const [dark, setDark] = useState(isDark);
  useEffect(() => {
    const mo = new MutationObserver(() => setDark(isDark()));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => mo.disconnect();
  }, []);
  const src = dark ? logoPixel : logoPixelLight;
  return <img src={src} width={size} height={size} className="logo" alt="" style={{ imageRendering: "pixelated" }} />;
}
