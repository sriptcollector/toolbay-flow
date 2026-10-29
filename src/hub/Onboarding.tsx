import { ReactNode, useEffect, useRef, useState } from "react";
import { api, LANGUAGES, on, Style } from "../api";
import { comboLabel, isMac } from "../keys";
import { askedThisRun, clearPermissionResume, markPermissionAsked, resumedFrom } from "./permResume";
import Credit from "./Credit";
import { Logo, TitleBar, useTheme } from "./Hub";
import { ActiveDownload, useModelStatus } from "./pages/Models";
import { useRecorder } from "./pages/Shortcuts";
import { Keys, Toggle, useSettings } from "./ui";

// Same order as the original: about you → permissions → set up → learn → personalize.
// Plans, trials, referrals and teams are left out on purpose: everything is free.
const STEPS = ["Welcome", "About you", "Where you type", "Privacy", "Permissions", "Microphone", "Language", "Shortcut", "Try it", "Your style", "Done"] as const;
const GROUPS: [string, number][] = [
  ["Welcome", 0],
  ["Permissions", 3],
  ["Set up", 5],
  ["Learn", 8],
  ["Personalize", 9],
];

const ROLES = ["Consultant", "Creator", "Customer support", "Data analysis", "Developer", "Educator", "Founder / CEO", "Healthcare", "Legal", "Marketing", "Operations", "Product", "Recruiting", "Sales", "Student", "Writer", "Other"];
const HEARD = ["Friend or colleague", "YouTube", "Social media", "Toolbay", "Search engine", "AI chat", "Newsletter", "Podcast", "Article", "Other"];
const USES: [string, string][] = [
  ["ai", "Chatting with AI"],
  ["code", "Coding with AI"],
  ["email", "Drafting emails"],
  ["messages", "Sending messages"],
  ["notes", "Taking notes"],
  ["docs", "Writing documents and posts"],
];

export default function Onboarding({ onDone }: { onDone: () => void }) {
  // Back on the permissions step after a restart for a permission (see permResume.ts).
  const [step, setStep] = useState(() => (resumedFrom() === "setup" ? 4 : 0));
  const [s, save] = useSettings();
  useTheme(s?.appearance);
  if (!s) return null;
  const next = () => setStep((x) => Math.min(x + 1, STEPS.length - 1));
  const back = () => setStep((x) => Math.max(0, x - 1));
  const finish = async () => {
    await save({ onboarded: true, show_bar: true });
    onDone();
  };
  const group = [...GROUPS].reverse().find(([, i]) => step >= i)?.[0];
  const props = { next, back };
  return (
    <div className="ob">
      <TitleBar>
        <Logo size={24} />
        <span className="ob-brand">Toolbay Flow</span>
      </TitleBar>
      <div className="ob-top">
        <div className="ob-steps">
          {GROUPS.map(([t], gi) => {
            const end = GROUPS[gi + 1]?.[1] ?? STEPS.length;
            return (
              <span key={t} className={group === t ? "cur" : step >= end ? "done" : ""}>
                {t}
              </span>
            );
          })}
        </div>
        <div className="ob-progress">
          <i style={{ transform: `scaleX(${step / (STEPS.length - 1)})` }} />
        </div>
      </div>
      <div className="ob-body">
        {step === 0 && <Welcome next={next} />}
        {step === 1 && <AboutYou {...props} />}
        {step === 2 && <WhereYouType {...props} />}
        {step === 3 && <Privacy {...props} />}
        {step === 4 && <Permissions {...props} />}
        {step === 5 && <Mic {...props} />}
        {step === 6 && <Language {...props} />}
        {step === 7 && <ShortcutStep {...props} />}
        {step === 8 && <TryIt {...props} />}
        {step === 9 && <YourStyle {...props} />}
        {step === 10 && <Done back={back} finish={finish} />}
      </div>
    </div>
  );
}

type StepProps = { next: () => void; back: () => void };

function Nav({ back, next, nextLabel = "Continue", disabled, extra }: { back?: () => void; next: () => void; nextLabel?: string; disabled?: boolean; extra?: ReactNode }) {
  return (
    <div className="ob-nav">
      {back ? (
        <button className="btn ghost" onClick={back}>
          Back
        </button>
      ) : (
        <span />
      )}
      <div className="ob-nav-r">
        {extra}
        <button className="btn primary big" disabled={disabled} onClick={next}>
          {nextLabel}
        </button>
      </div>
    </div>
  );
}

function Chips({ options, value, onChange, multi }: { options: [string, string][]; value: string[]; onChange: (v: string[]) => void; multi?: boolean }) {
  return (
    <div className="chips">
      {options.map(([id, label]) => {
        const sel = value.includes(id);
        return (
          <button key={id} className={sel ? "sel" : ""} onClick={() => onChange(multi ? (sel ? value.filter((x) => x !== id) : [...value, id]) : [id])}>
            {label}
          </button>
        );
      })}
    </div>
  );
}

function useProfile() {
  const [s, save] = useSettings();
  const p = (s?.profile ?? {}) as Record<string, string[]>;
  return [p, (patch: Record<string, string[]>) => save({ profile: { ...p, ...patch } })] as const;
}

function Welcome({ next }: { next: () => void }) {
  return (
    <div className="ob-card welcome">
      <Logo size={44} />
      <h1 className="display">Welcome to Toolbay Flow</h1>
      <p className="lead">
        Toolbay Flow turns what you say into clean, punctuated text in any app: email, chat, docs, code, AI prompts. It runs on this computer, it's free, and the code is
        open.
      </p>
      <ul className="ticks">
        <li>Hold a key, speak, let go. Your words appear where your cursor is.</li>
        <li>Ums, repeats and false starts are removed. Lists and punctuation are added.</li>
        <li>No account, no limits, no subscription.</li>
      </ul>
      <Nav next={next} nextLabel="Get started" />
      <Credit />
    </div>
  );
}

function AboutYou({ next, back }: StepProps) {
  const [p, set] = useProfile();
  return (
    <div className="ob-card wide">
      <h1>Tell us about yourself</h1>
      <p>This helps Toolbay Flow pick good defaults. It's kept on this computer, and only goes to your Toolbay account if you turn on sync later.</p>
      <h4>What do you do for work?</h4>
      <Chips options={ROLES.map((r) => [r, r])} value={p.role ?? []} onChange={(v) => set({ role: v })} />
      <h4>Where did you hear about Toolbay Flow?</h4>
      <Chips options={HEARD.map((r) => [r, r])} value={p.heard ?? []} onChange={(v) => set({ heard: v })} />
      <Nav
        back={back}
        next={next}
        extra={
          <button className="link" onClick={next}>
            Skip
          </button>
        }
      />
    </div>
  );
}

function WhereYouType({ next, back }: StepProps) {
  const [p, set] = useProfile();
  return (
    <div className="ob-card wide">
      <h1>Where do you spend time typing?</h1>
      <p>Pick all that apply.</p>
      <Chips multi options={USES} value={p.uses ?? []} onChange={(v) => set({ uses: v })} />
      <Nav
        back={back}
        next={next}
        extra={
          <button className="link" onClick={next}>
            Skip
          </button>
        }
      />
    </div>
  );
}

function Privacy({ next, back }: StepProps) {
  const [s, save] = useSettings();
  if (!s) return null;
  return (
    <div className="ob-card">
      <h1>Your voice turns into text on this computer</h1>
      <p>
        Speech recognition and cleanup run on your own computer with open models. Nothing is sent anywhere unless you turn on sync or a cloud AI option. No analytics.
      </p>
      <div className="card">
        <div className="row">
          <div className="row-text">
            <div className="row-title">Context awareness</div>
            <div className="row-desc">Read the text near your cursor so what you say fits in. Never reads password fields.</div>
          </div>
          <Toggle on={s.context_awareness} onChange={(v) => save({ context_awareness: v })} />
        </div>
        <div className="row">
          <div className="row-text">
            <div className="row-title">Keep a history of my dictations</div>
            <div className="row-desc">Kept on this computer so you can copy or fix past text. Only synced if you turn that on in Settings.</div>
          </div>
          <Toggle on={s.data_policy !== "never_store"} onChange={(v) => save({ data_policy: v ? "store_normally" : "never_store" })} />
        </div>
      </div>
      <Nav back={back} next={next} />
    </div>
  );
}

function PermCard({ icon, title, desc, ok, action, onAction }: { icon: string; title: string; desc: string; ok: boolean | null; action: string; onAction: () => void }) {
  return (
    <div className={`perm ${ok ? "ok" : ""}`}>
      <div className="perm-icon">{icon}</div>
      <div className="perm-text">
        <b>{title}</b>
        <span>{desc}</span>
      </div>
      {ok ? (
        <span className="status ok">Allowed</span>
      ) : (
        <button className="btn" onClick={onAction}>
          {action}
        </button>
      )}
    </div>
  );
}

function Permissions({ next, back }: StepProps) {
  const [mic, setMic] = useState<{ ok: boolean; message: string } | null>(null);
  const [mac, setMac] = useState<{ input: boolean } | null>(null);
  const [resumed] = useState(() => resumedFrom() === "setup");
  const [asked, setAsked] = useState(askedThisRun);
  const allow = (kind: string) => {
    markPermissionAsked("setup");
    setAsked(true);
    api.openSystemSettings(kind);
  };
  const macOk = !isMac || mac?.input;
  useEffect(() => {
    if (resumed && macOk) clearPermissionResume();
  }, [resumed, macOk]);
  useEffect(() => {
    const check = () => {
      api.micCheck().then(setMic);
      if (isMac) api.macPermissions(false).then(setMac);
    };
    check();
    const t = setInterval(check, 2500);
    return () => clearInterval(t);
  }, []);
  return (
    <div className="ob-card wide">
      <h1>Allow Toolbay Flow to hear you</h1>
      <p>{isMac ? "macOS asks you to allow these once. Click each button, then switch Toolbay Flow on in the window that opens." : "Windows needs to let apps use your microphone."}</p>
      <div className="perms">
        <PermCard
          icon="🎙️"
          title="Microphone"
          desc={mic && !mic.ok ? mic.message : "So Toolbay Flow can hear you while you hold the key."}
          ok={mic?.ok ?? null}
          action="Open settings"
          onAction={() => api.openSystemSettings("microphone")}
        />
        {isMac && (
          <>
            <PermCard icon="⌨️" title="Input Monitoring" desc="So Toolbay Flow notices when you hold your shortcut. It only sees keys after your Mac has used them; it can never press or block a key. Your text lands on the clipboard: press ⌘V to paste." ok={mac?.input ?? null} action="Allow" onAction={() => allow("input")} />
          </>
        )}
      </div>
      {isMac && mac && resumed && (
        <div className={`perm-result ${macOk ? "ok" : ""}`} role="status">
          {macOk
            ? "Restarted. Both permissions are on, so your shortcut works now."
            : "Restarted, but macOS still has a permission off. Switch Toolbay Flow on for the one that isn't Allowed, then restart again."}
        </div>
      )}
      {isMac && mac && !macOk && asked && (
        <div className="perm-restart">
          <span>Switched them on? macOS applies them after Toolbay Flow restarts. You'll come right back here.</span>
          <button className="btn" onClick={() => api.restartApp()}>
            Restart Toolbay Flow
          </button>
        </div>
      )}
      {isMac && (
        <div className="howto">
          <div className="howto-pic">
            <div className="fake-row">
              <span>Toolbay Flow</span>
              <span className="fake-toggle on" />
            </div>
          </div>
          <span className="muted small">In System Settings, find Toolbay Flow in the list and turn it on. If it isn't listed, click + and pick Toolbay Flow from Applications.</span>
        </div>
      )}
      <Nav back={back} next={next} nextLabel={mic?.ok && (!isMac || mac?.input) ? "Continue" : "I've allowed it"} />
    </div>
  );
}

function Mic({ next, back }: StepProps) {
  const [s, save] = useSettings();
  const [mics, setMics] = useState<string[]>([]);
  const [lvl, setLvl] = useState<number[]>(Array(24).fill(0));
  const [heard, setHeard] = useState(false);
  const [err, setErr] = useState("");
  const [p, set] = useProfile();
  useEffect(() => {
    api.listMics().then(setMics);
    const off = on<number>("mic-level", (l) => {
      setLvl((a) => [...a.slice(1), l]);
      if (l > 0.15) setHeard(true);
    });
    return () => {
      off();
      api.micTest(false);
    };
  }, []);
  useEffect(() => {
    if (!s) return;
    setErr("");
    api.micTest(false).then(() => api.micTest(true)).catch((e) => setErr(String(e)));
  }, [s?.microphone]);
  if (!s) return null;
  return (
    <div className="ob-card wide">
      <h1>Test your microphone</h1>
      <p>Say a few words. Do you see the bars move while you speak? Built-in and wired microphones work best; Bluetooth headsets can be less reliable.</p>
      <select value={s.microphone ?? ""} onChange={(e) => save({ microphone: e.target.value || null })}>
        <option value="">System default</option>
        {mics.map((m) => (
          <option key={m}>{m}</option>
        ))}
      </select>
      <div className={`meter ${heard ? "ok" : ""}`}>
        {lvl.map((l, i) => (
          <i key={i} style={{ transform: `scaleY(${Math.max(0.06, Math.min(1, l * 1.4))})` }} />
        ))}
      </div>
      {err && <p className="warn">{err}</p>}
      <h4>Do you work next to other people?</h4>
      <Chips
        options={[
          ["yes", "Yes"],
          ["no", "No"],
        ]}
        value={p.near ?? []}
        onChange={(v) => set({ near: v })}
      />
      {p.near?.[0] === "yes" && <p className="muted small">Tip: speak at a normal, quiet volume. A headset or earbuds mic picks up less of the room.</p>}
      <Nav back={back} next={next} nextLabel={heard ? "Yes, I see them" : "Continue"} />
    </div>
  );
}

function Language({ next, back }: StepProps) {
  const [s, save] = useSettings();
  if (!s) return null;
  return (
    <div className="ob-card wide">
      <h1>Which language do you speak?</h1>
      <p>Pick one for the best accuracy, or let Toolbay Flow detect it every time.</p>
      <div className="langs">
        {LANGUAGES.slice(0, 30).map(([c, n]) => (
          <button key={c} className={s.language === c ? "sel" : ""} onClick={() => save({ language: c }).then(() => api.rerunAuto())}>
            {n}
          </button>
        ))}
      </div>
      <Nav back={back} next={next} />
    </div>
  );
}

function ShortcutStep({ next, back }: StepProps) {
  const [s, save] = useSettings();
  const [ok, setOk] = useState(false);
  const [changing, setChanging] = useState(false);
  const ptt = s?.shortcuts.find((x) => x.action === "push_to_talk");
  const rec = useRecorder((keys) => {
    if (!s) return;
    if (changing) {
      save({ shortcuts: s.shortcuts.map((x) => (x.action === "push_to_talk" ? { ...x, keys: [...keys].sort((a, b) => a - b) } : x)) });
      setChanging(false);
      setOk(true);
    } else if (ptt && [...keys].sort().join() === [...ptt.keys].sort().join()) {
      setOk(true);
    }
    setTimeout(() => rec.start(), 50);
  });
  useEffect(() => {
    rec.start();
    return () => rec.stop();
  }, []);
  if (!s || !ptt) return null;
  return (
    <div className="ob-card">
      <h1>Press the shortcut to try it</h1>
      <p>
        You'll hold <b>{comboLabel(ptt.keys)}</b>, speak, and let go. Press it now. Do the keys light up while you hold them?
      </p>
      <div className={`bigkeys ${ok ? "ok" : ""}`}>
        <Keys keys={changing ? rec.keys : ptt.keys} live={new Set(rec.keys)} />
      </div>
      <p className="muted small">{changing ? "Press the new combination, then let go." : ok ? "That works." : "Waiting for you to press it…"}</p>
      <button className="link" onClick={() => setChanging(true)}>
        Use a different shortcut
      </button>
      <div className="tips">
        <div>
          <b>Hands-free:</b> double-tap the shortcut, talk as long as you like, then tap it again.
        </div>
        <div>
          <b>Cancel:</b> press Esc while Flow is listening.
        </div>
      </div>
      <Nav back={back} next={next} nextLabel={ok ? "Yes, they light up" : "Continue"} />
    </div>
  );
}

const PRACTICE: { kind: "chat" | "email" | "doc"; title: string; say: string }[] = [
  { kind: "chat", title: "Send a quick message", say: "Hey, um, are we still on for lunch tomorrow? I can do 12, no wait, 1 works better." },
  { kind: "email", title: "Write an email", say: "Hi Sam, thanks for the update. The numbers look good and I'll send the contract by Friday. Best, Alex" },
  { kind: "doc", title: "Take a note", say: "Ideas for the launch: a short demo video, a post on the community page, and a thank you email to the beta testers." },
];

function TryIt({ next, back }: StepProps) {
  const [s] = useSettings();
  const [i, setI] = useState(0);
  const [text, setText] = useState(["", "", ""]);
  const ta = useRef<HTMLTextAreaElement>(null);
  const status = useModelStatus();
  useEffect(() => {
    ta.current?.focus();
  }, [i]);
  const ptt = s?.shortcuts.find((x) => x.action === "push_to_talk");
  const p = PRACTICE[i];
  const set = (v: string) => setText((t) => t.map((x, j) => (j === i ? v : x)));
  return (
    <div className="ob-card wide">
      <h1>{p.title}</h1>
      <p>
        Click in the box, hold <b>{ptt ? comboLabel(ptt.keys) : "your shortcut"}</b> and read this out loud, ums and all:
      </p>
      <blockquote>“{p.say}”</blockquote>
      <div className={`mock mock-${p.kind}`}>
        {p.kind === "chat" && (
          <>
            <div className="mock-head">
              <span className="dot" /> Jordan
            </div>
            <div className="bubble-in">Lunch tomorrow?</div>
          </>
        )}
        {p.kind === "email" && (
          <div className="mock-head col">
            <span>To: sam@example.com</span>
            <span>Subject: Contract</span>
          </div>
        )}
        {p.kind === "doc" && <div className="mock-head">Launch notes</div>}
        <textarea ref={ta} rows={p.kind === "chat" ? 2 : 5} value={text[i]} onChange={(e) => set(e.target.value)} placeholder={p.kind === "chat" ? "Message" : "Start talking…"} />
      </div>
      {(status.busy || status.message) && (
        <div className="muted small">
          {status.message || "Getting faster models ready…"} You can already dictate.
          <ActiveDownload />
        </div>
      )}
      <Nav
        back={i === 0 ? back : () => setI(i - 1)}
        next={i < PRACTICE.length - 1 ? () => setI(i + 1) : next}
        nextLabel={text[i].trim() ? "Next" : "Skip"}
        extra={
          <span className="muted small">
            {i + 1} of {PRACTICE.length}
          </span>
        }
      />
    </div>
  );
}

const STYLE_OPTS: [Style, string][] = [
  ["formal", "Formal"],
  ["casual", "Casual"],
  ["very_casual", "Very casual"],
  ["excited", "Excited"],
];

function YourStyle({ next, back }: StepProps) {
  const [s, save] = useSettings();
  if (!s) return null;
  const rows: ["personal" | "work" | "email" | "other", string][] = [
    ["personal", "Personal messages"],
    ["work", "Work messages"],
    ["email", "Email"],
    ["other", "Everything else"],
  ];
  return (
    <div className="ob-card wide">
      <h1>How should you sound?</h1>
      <p>Toolbay Flow adjusts to the app you're in. Change this any time under Style. Styles apply to English.</p>
      {rows.map(([k, label]) => (
        <div key={k} className="style-pick">
          <b>{label}</b>
          <Chips options={STYLE_OPTS} value={[s.styles[k]]} onChange={(v) => save({ styles: { ...s.styles, [k]: v[0] as Style } })} />
        </div>
      ))}
      <Nav back={back} next={next} />
    </div>
  );
}

function Done({ back, finish }: { back: () => void; finish: () => void }) {
  const [s] = useSettings();
  if (!s) return null;
  const k = (a: string) => s.shortcuts.find((x) => x.action === a)?.keys ?? [];
  return (
    <div className="ob-card">
      <h1>You're all set</h1>
      <p>Keep Toolbay Flow open while you dictate. The small bar at the bottom of your screen shows when it is listening.</p>
      <div className="card summary">
        <div className="row">
          <span>Dictate</span>
          <Keys keys={k("push_to_talk")} />
        </div>
        <div className="row">
          <span>Hands-free</span>
          <Keys keys={k("hands_free")} />
        </div>
        <div className="row">
          <span>Edit selected text by voice</span>
          <Keys keys={k("command")} />
        </div>
        <div className="row">
          <span>Paste your last dictation</span>
          <Keys keys={k("paste_last")} />
        </div>
      </div>
      <Nav back={back} next={finish} nextLabel="Start using Toolbay Flow" />
    </div>
  );
}
