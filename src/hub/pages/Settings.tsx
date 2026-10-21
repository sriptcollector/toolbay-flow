import { useEffect, useState } from "react";
import { api, CleanupLevel, DataPolicy, LANGUAGES, Provider } from "../../api";
import { Page, Row, Section, Toggle, useSettings } from "../ui";
import { isMac } from "../../keys";
import Credit from "../Credit";

const LEVELS: [CleanupLevel, string, string][] = [
  ["none", "Off", "Paste exactly what was heard"],
  ["light", "Light", "Remove fillers, add punctuation (instant, no AI)"],
  ["medium", "Standard", "AI fixes punctuation, fillers, corrections, lists"],
  ["high", "Polished", "Standard, plus tighter wording"],
];

export default function SettingsPage() {
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
        <Row title="Launch at login" desc="Start Flow quietly in the tray when you sign in.">
          <Toggle on={s.launch_at_login} onChange={(v) => save({ launch_at_login: v })} />
        </Row>
        <ShortcutToggle />
        <Row title="Show the Flow bar" desc="The small pill at the bottom of your screen.">
          <Toggle on={s.show_bar} onChange={(v) => save({ show_bar: v })} />
        </Row>
        <Row title="Sounds" desc="Short tones when dictation starts and stops.">
          <Toggle on={s.sounds} onChange={(v) => save({ sounds: v })} />
        </Row>
        <Row title="Keep my clipboard" desc="Put back what was on the clipboard after Flow pastes.">
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
        <Row title="Context awareness" desc="Read the text around your cursor in the focused app so Flow continues it naturally. Never reads password fields. Stays on this computer.">
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

      <Section title="Data and privacy">
        <Row title="History" desc="Where your dictations are kept. Everything stays on this computer either way.">
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
        <Row title="No telemetry" desc="Flow sends nothing anywhere. The only network use is downloading models you choose, and your own API provider if you pick one." />
      </Section>

      <Section title="About">
        <Row title={`Toolbay Flow ${info?.version ?? ""}`} desc="Free and open source under the MIT licence. Part of the Toolbay collection of free tools.">
          <a className="link" href="https://github.com/sriptcollector/toolbay-flow" target="_blank" rel="noreferrer">
            Source code
          </a>
        </Row>
        <Row title="Run setup again">
          <button className="btn" onClick={() => save({ onboarded: false })}>
            Start setup
          </button>
        </Row>
        <Row title="Quit Flow" desc="Closing this window keeps Flow running in the tray.">
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
