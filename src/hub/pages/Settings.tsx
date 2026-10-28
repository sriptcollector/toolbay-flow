import { useEffect, useState } from "react";
import { api, CleanupLevel, on, SyncStatus, DataPolicy, LANGUAGES, Provider } from "../../api";
import { Page, Row, Section, Toggle, useSettings } from "../ui";
import { isMac } from "../../keys";
import Credit from "../Credit";

const LEVELS: [CleanupLevel, string, string][] = [
  ["none", "Off", "Paste exactly what was heard"],
  ["light", "Light", "Remove fillers, add punctuation (instant, no AI)"],
  ["medium", "Standard", "AI fixes punctuation, fillers, corrections, lists"],
  ["high", "Polished", "Standard, plus tighter wording"],
];

export default function SettingsPage({ go }: { go: (p: string) => void }) {
  const [s, save] = useSettings();
  const [mics, setMics] = useState<string[]>([]);
  const [info, setInfo] = useState<{ version: string } | null>(null);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => {
    api.listMics().then(setMics);
    api.appInfo().then(setInfo);
  }, []);
  if (!s) return null;

  return (
    <Page title="Settings">
      <Section title="General">
        <Row title="Appearance" desc="Light like paper, or dark for late nights.">
          <select value={s.appearance} onChange={(e) => save({ appearance: e.target.value as "light" | "dark" | "system" })}>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
            <option value="system">Match my computer</option>
          </select>
        </Row>
        <ShortcutToggle />
        <Row title="Show the dictation bar" desc="The small pill at the bottom of your screen.">
          <Toggle on={s.show_bar} onChange={(v) => save({ show_bar: v })} />
        </Row>
        <Row title="Sounds" desc="Short tones when dictation starts and stops.">
          <Toggle on={s.sounds} onChange={(v) => save({ sounds: v })} />
        </Row>
        <Row title="Keep my clipboard" desc="Put back what was on the clipboard after Toolbay Flow pastes.">
          <Toggle on={s.restore_clipboard} onChange={(v) => save({ restore_clipboard: v })} />
        </Row>
        <Row title="Your name" desc="Used to sign emails you dictate.">
          <input className="inline" value={s.name} placeholder="Optional" onChange={(e) => save({ name: e.target.value })} />
        </Row>
      </Section>

      <Section title="Voice">
        <Row title="Microphone">
          <select value={s.microphone ?? ""} onChange={(e) => save({ microphone: e.target.value || null })}>
            <option value="">System default</option>
            {mics.map((m) => (
              <option key={m}>{m}</option>
            ))}
          </select>
        </Row>
        <Row title="Language" desc="Auto-detect needs a multilingual speech model.">
          <select value={s.language} onChange={(e) => save({ language: e.target.value })}>
            {LANGUAGES.map(([c, n]) => (
              <option key={c} value={c}>
                {n}
              </option>
            ))}
          </select>
        </Row>
      </Section>

      <Section title="Cleanup">
        <div className="levels">
          {LEVELS.map(([id, label, desc]) => (
            <button key={id} className={`level ${s.cleanup_level === id ? "sel" : ""}`} onClick={() => save({ cleanup_level: id })}>
              <b>{label}</b>
              <span>{desc}</span>
            </button>
          ))}
        </div>
        <Row title="AI model" desc="Local runs on this computer. Cloud options use your own API key and send the transcript to that provider.">
          <select value={s.llm_provider} onChange={(e) => save({ llm_provider: e.target.value as Provider })}>
            <option value="local">On this computer</option>
            <option value="open_ai_compatible">OpenAI-compatible API (your key)</option>
            <option value="anthropic">Anthropic API (your key)</option>
          </select>
        </Row>
        {s.llm_provider === "open_ai_compatible" && (
          <div className="subform">
            <label>
              Base URL
              <input value={s.cloud_llm.base_url} onChange={(e) => save({ cloud_llm: { ...s.cloud_llm, base_url: e.target.value } })} />
            </label>
            <label>
              Model
              <input value={s.cloud_llm.model} onChange={(e) => save({ cloud_llm: { ...s.cloud_llm, model: e.target.value } })} />
            </label>
            <label>
              API key
              <input type="password" value={s.cloud_llm.api_key} onChange={(e) => save({ cloud_llm: { ...s.cloud_llm, api_key: e.target.value } })} />
            </label>
          </div>
        )}
        {s.llm_provider === "anthropic" && (
          <div className="subform">
            <label>
              Model
              <input value={s.anthropic.model} onChange={(e) => save({ anthropic: { ...s.anthropic, model: e.target.value } })} />
            </label>
            <label>
              API key
              <input type="password" value={s.anthropic.api_key} onChange={(e) => save({ anthropic: { ...s.anthropic, api_key: e.target.value } })} />
            </label>
          </div>
        )}
        <Row title="Context awareness" desc="Read the text around your cursor in the focused app so Toolbay Flow continues it naturally. Never reads password fields. Stays on this computer, unless you pick a cloud AI option: then it goes to that provider along with your words.">
          <Toggle on={s.context_awareness} onChange={(v) => save({ context_awareness: v })} />
        </Row>
        <Row title="Command mode" desc="Hold the command shortcut to edit selected text by voice, or to ask for a reply.">
          <Toggle on={s.command_mode} onChange={(v) => save({ command_mode: v })} />
        </Row>
      </Section>

      <Section title="Experimental">
        <Row title="“Press enter” voice command" desc="End a dictation with “press enter” to paste and send it, in chat apps for example.">
          <Toggle on={s.press_enter_command} onChange={(v) => save({ press_enter_command: v })} />
        </Row>
        <Row
          title="Save debug recordings"
          desc="If words go missing, this keeps the audio, what the speech model heard and the final text for your last 20 dictations, in a debug folder on this computer, so you can see which step lost them."
        >
          {s.debug_recordings && (
            <button className="link" onClick={() => api.openDataDir()}>
              Open folder
            </button>
          )}
          <Toggle on={s.debug_recordings} onChange={(v) => save({ debug_recordings: v })} />
        </Row>
      </Section>

      <AccountSync />

      <Section title="Data and privacy">
        <Row title="History" desc="How long your dictations are kept on this computer.">
          <select value={s.data_policy} onChange={(e) => save({ data_policy: e.target.value as DataPolicy })}>
            <option value="store_normally">Keep history</option>
            <option value="delete_after24h">Delete after 24 hours</option>
            <option value="never_store">Don't keep history</option>
          </select>
        </Row>
        <Row title="Keep recordings for 14 days" desc="Lets you re-run a dictation with a better model. Older audio is deleted automatically.">
          <Toggle on={s.keep_audio} onChange={(v) => save({ keep_audio: v })} />
        </Row>
        <Row title="Delete all history">
          {confirm ? (
            <>
              <button className="btn danger" onClick={() => api.historyClear().then(() => setConfirm(false))}>
                Delete everything
              </button>
              <button className="link" onClick={() => setConfirm(false)}>
                Keep it
              </button>
            </>
          ) : (
            <button className="btn" onClick={() => setConfirm(true)}>
              Delete…
            </button>
          )}
        </Row>
        <Row title="No telemetry" desc="Toolbay Flow never sends usage data. It only goes online to download models and updates, to reach your own API provider if you pick a cloud AI option, to post feedback you choose to send, and to sync with your Toolbay account if you turn sync on." />
      </Section>

      <Section title="About">
        <Row title={`Toolbay Flow ${info?.version ?? ""}`} desc="Free and open source under the MIT licence. Part of the Toolbay collection of free tools.">
          <a className="link" href="https://github.com/sriptcollector/toolbay-flow" target="_blank" rel="noreferrer">
            Source code
          </a>
        </Row>
        <Row title="Share feedback" desc="Tell the maker what works, what's broken, or what you'd like next. Posts to toolbay.ai.">
          <button className="btn" onClick={() => go("feedback")}>
            Share feedback
          </button>
        </Row>
        <Row title="Run setup again">
          <button className="btn" onClick={() => save({ onboarded: false })}>
            Start setup
          </button>
        </Row>
        <Row title="Quit Toolbay Flow" desc="Closing this window also quits Toolbay Flow. Nothing keeps running in the background.">
          <button className="btn" onClick={() => api.quit()}>
            Quit
          </button>
        </Row>
      </Section>
      <footer className="settings-foot">
        <Credit />
      </footer>
    </Page>
  );
}

function ago(iso: string | null) {
  if (!iso) return "";
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  return new Date(iso).toLocaleString();
}

/**
 * Toolbay account sync. Off until the person turns it on. When on, the synced
 * data sits on toolbay.ai where Toolbay's server can read it (not end-to-end
 * encrypted), and the copy says so.
 */
function AccountSync() {
  const [s, save] = useSettings();
  const [st, setSt] = useState<SyncStatus | null>(null);
  const [signedIn, setSignedIn] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [deleted, setDeleted] = useState(false);
  const [err, setErr] = useState("");
  useEffect(() => {
    api.syncStatus().then(setSt);
    api.syncSignedIn().then(setSignedIn).catch(() => {});
    const off = on<SyncStatus>("sync-status", (x) => {
      setSt(x);
      if (x.account) setWaiting(false);
    });
    const focus = () => {
      api.syncSignedIn().then(setSignedIn).catch(() => {});
      // Does nothing (no network) while sync is off.
      api.syncNow().then(setSt);
    };
    window.addEventListener("focus", focus);
    return () => {
      off();
      window.removeEventListener("focus", focus);
    };
  }, []);
  useEffect(() => {
    if (!waiting) return;
    const t = setInterval(
      () =>
        api.syncSignedIn().then((v) => {
          setSignedIn(v);
          if (v) setWaiting(false);
        }),
      3000,
    );
    return () => clearInterval(t);
  }, [waiting]);
  if (!s || !st) return null;
  const who = st.account ? st.account.name || (st.account.handle ? `@${st.account.handle}` : "your Toolbay account") : "";
  const signIn = () => {
    setErr("");
    setWaiting(true);
    api.syncSignIn().catch((e) => {
      setWaiting(false);
      setErr(String(e));
    });
  };
  const toggle = (v: boolean) => {
    setDeleted(false);
    save({ sync_enabled: v, sync_asked: true }).then(() => {
      if (v) api.syncNow().then(setSt);
    });
  };
  return (
    <Section title="Toolbay account">
      {!signedIn && !st.account ? (
        <Row
          title="Sync across your computers (optional)"
          desc={
            err ||
            (waiting
              ? "Finish signing in with the Toolbay app. Toolbay Flow picks it up by itself."
              : "Sign in with your Toolbay account, then turn on sync if you want your settings, dictionary and snippets on every Mac and PC. Signing in alone sends nothing. Toolbay Flow works fully without it.")
          }
        >
          <button className="btn primary" onClick={signIn} disabled={waiting}>
            {waiting ? "Waiting…" : "Sign in"}
          </button>
        </Row>
      ) : (
        <>
          <Row
            title={who ? `Signed in as ${who}` : "Signed in to Toolbay"}
            desc={
              !s.sync_enabled
                ? "Sync is off. Nothing is synced to Toolbay from this computer."
                : st.message ||
                  (st.state === "syncing"
                    ? "Syncing…"
                    : st.synced_at
                      ? `Up to date, synced ${ago(st.synced_at)}.${st.pending ? ` ${st.pending} change${st.pending > 1 ? "s" : ""} waiting.` : ""}`
                      : "Getting ready to sync…")
            }
          >
            {s.sync_enabled && (
              <button className="link" onClick={() => api.syncNow().then(setSt)}>
                Sync now
              </button>
            )}
          </Row>
          <Row
            title="Sync across my computers"
            desc="Sends your name, setup answers, settings, styles, shortcuts, transforms, dictionary and snippets to your Toolbay account on toolbay.ai. Toolbay's server can read this data (it is not end-to-end encrypted). Your recordings and API keys are never sent. Off unless you turn it on."
          >
            <Toggle on={s.sync_enabled} onChange={toggle} />
          </Row>
          {s.sync_enabled && (
            <Row
              title="Also sync my recent dictations"
              desc="Also sends the text of your last 50 dictations (never audio) to toolbay.ai, where Toolbay's server can read it, so they show up on your other computers."
            >
              <Toggle on={s.sync_history} onChange={(v) => save({ sync_history: v })} />
            </Row>
          )}
          <Row
            title="Delete my synced data from Toolbay"
            desc={
              deleted
                ? "Done. Toolbay no longer has a synced copy, and sync is off. Everything is still on this computer."
                : "Deletes everything Toolbay Flow synced from your Toolbay account and turns sync off. Your copy on this computer is kept."
            }
          >
            {confirm ? (
              <>
                <button
                  className="btn danger"
                  onClick={() =>
                    api
                      .syncDeleteRemote()
                      .then(() => {
                        setConfirm(false);
                        setDeleted(true);
                      })
                      .catch((e) => {
                        setConfirm(false);
                        setErr(String(e));
                      })
                  }
                >
                  Delete
                </button>
                <button className="link" onClick={() => setConfirm(false)}>
                  Keep it
                </button>
              </>
            ) : (
              <button className="btn" onClick={() => setConfirm(true)}>
                Delete…
              </button>
            )}
          </Row>
          {err && <div className="pad-row muted small">{err}</div>}
        </>
      )}
    </Section>
  );
}

function ShortcutToggle() {
  const [on, setOn] = useState<boolean | null>(null);
  const [err, setErr] = useState("");
  const [hub, setHub] = useState(false);
  useEffect(() => {
    api.hasDesktopShortcut().then(setOn);
    api.appInfo().then((i) => setHub(i.fromToolbay));
  }, []);
  // Under the Toolbay app, the hub owns the desktop shortcut.
  if (on === null || hub) return null;
  return (
    <Row title="Desktop shortcut" desc={err || (isMac ? "A Toolbay Flow icon on your desktop." : "A Toolbay Flow icon on your desktop.")}>
      <Toggle
        on={on}
        onChange={(v) =>
          api
            .setDesktopShortcut(v)
            .then(() => setOn(v))
            .catch((e) => setErr(String(e)))
        }
      />
    </Row>
  );
}
