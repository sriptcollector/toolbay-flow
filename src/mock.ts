// Browser-only preview with sample data (open http://localhost:1437/?mock).
// Lets the UI be developed and screenshotted without the Rust side.
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const now = Date.now();
const iso = (minAgo: number) => new Date(now - minAgo * 60000).toISOString();

const settings = {
  onboarded: !location.search.includes("onboarding"),
  show_bar: true,
  sounds: true,
  model_mode: "auto",
  hw_fingerprint: "",
  auto_reason: "NVIDIA GeForce RTX 4070 Ti with 11 GB",
  auto_slow_speech: false,
  auto_slow_cleanup: false,
  desktop_shortcut: true,
  profile: null,
  press_enter_command: true,
  bar_snooze_until: 0,
  debug_recordings: false,
  appearance: "light",
  sync_enabled: false,
  sync_asked: false,
  sync_history: false,
  engine: "local",
  cloud_asr: { base_url: "https://api.groq.com/openai/v1", api_key: "", model: "whisper-large-v3-turbo" },
  microphone: null,
  language: "en",
  speech_model: "large-v3-turbo-q5",
  use_gpu: true,
  cleanup_level: "medium",
  cleanup_chosen: false,
  llm_provider: "local",
  local_llm_model: "qwen3-4b-instruct",
  cloud_llm: { base_url: "https://api.openai.com/v1", api_key: "", model: "gpt-4o-mini" },
  anthropic: { base_url: "https://api.anthropic.com", api_key: "", model: "claude-haiku-4-5" },
  styles: { personal: "casual", work: "formal", email: "formal", other: "formal" },
  context_awareness: true,
  auto_learn_words: true,
  command_mode: true,
  data_policy: "store_normally",
  keep_audio: true,
  shortcuts: [
    { action: "push_to_talk", keys: [0x11, 0x5b] },
    { action: "hands_free", keys: [0x11, 0x20, 0x5b] },
    { action: "command", keys: [0x11, 0x12, 0x5b] },
    { action: "paste_last", keys: [0x10, 0x12, 0x5a] },
    { action: "copy_last", keys: [0x10, 0x12, 0x58] },
    { action: "dismiss", keys: [0x1b] },
    { action: "transform1", keys: [0x12, 0x31, 0x5b] },
    { action: "transform2", keys: [0x12, 0x32, 0x5b] },
    { action: "transform3", keys: [0x12, 0x33, 0x5b] },
  ],
  transforms: [
    { id: "polish", name: "Polish", instructions: "Rewrite the text so it reads clearly and naturally. Fix grammar, remove repetition and filler, keep the author's voice, meaning and language. Do not add new information." },
    { id: "prompt", name: "Prompt engineer", instructions: "Turn the text into a clear, well-structured prompt for an AI assistant: state the goal, the context, constraints and the desired output format." },
    { id: "concise", name: "Make it shorter", instructions: "Make the text about half as long while keeping every important point and the author's tone." },
  ],
  app_categories: { "notepad.exe": "other" },
  restore_clipboard: true,
  name: "Orion",
};

const history = [
  { id: 6, created_at: iso(3), mode: "dictation", raw_text: "um so I think we should uh meet on Tuesday at 6 no wait 7 and talk about the the roadmap", text: "So I think we should meet on Tuesday at 7 and talk about the roadmap.", app: "Slack", words: 15, has_audio: true },
  { id: 5, created_at: iso(9), mode: "dictation", raw_text: "things I need from the store eggs milk uh bread and coffee", text: "Things I need from the store:\n- Eggs\n- Milk\n- Bread\n- Coffee", app: "Notepad", words: 10, has_audio: true },
  { id: 4, created_at: iso(14), mode: "dictation", raw_text: "Hi Dana thanks for sending the notes over I will review them tonight and get back to you tomorrow best Orion", text: "Hi Dana,\n\nThanks for sending the notes over. I will review them tonight and get back to you tomorrow.\n\nBest,\nOrion", app: "Outlook", words: 21, has_audio: true },
  { id: 3, created_at: iso(40), mode: "command", raw_text: "make this friendlier", text: "Hey team! Quick heads-up: the launch moves to Thursday. Thanks for rolling with it!", app: "Google Chrome", words: 15, has_audio: false },
  { id: 2, created_at: iso(60 * 26), mode: "dictation", raw_text: "hey are we still on for dinner tonight I can uh bring dessert", text: "Hey, are we still on for dinner tonight? I can bring dessert", app: "WhatsApp", words: 12, has_audio: true },
  { id: 1, created_at: iso(60 * 27), mode: "transform", raw_text: "write a prompt for summarising", text: "Summarize the meeting notes below in five bullet points…", app: "Claude", words: 9, has_audio: false },
].map((h) => ({ window_title: "", url: "", language: "en", duration_ms: 6000, latency_ms: 420, status: "pasted", edited_text: null, ...h }));

const dict = [
  { id: 1, phrase: "Toolbay", replacement: null, is_snippet: false, starred: true, source: "manual", uses: 31, created_at: iso(5000) },
  { id: 2, phrase: "Kubernetes", replacement: null, is_snippet: false, starred: false, source: "user_edits", uses: 4, created_at: iso(900) },
  { id: 3, phrase: "west ward", replacement: "Westward", is_snippet: false, starred: false, source: "manual", uses: 12, created_at: iso(4000) },
  { id: 4, phrase: "Siobhan", replacement: null, is_snippet: false, starred: false, source: "manual", uses: 2, created_at: iso(3000) },
];
const snippets = [
  { id: 10, phrase: "my calendar link", replacement: "https://cal.com/orion/30min", is_snippet: true, starred: false, source: "manual", uses: 9, created_at: iso(2000) },
  { id: 11, phrase: "my signature", replacement: "Best,\nOrion Jones\nToolbay · toolbay.ai", is_snippet: true, starred: false, source: "manual", uses: 22, created_at: iso(2000) },
  { id: 12, phrase: "review prompt", replacement: "Review this pull request for correctness bugs first, then readability. List each issue with file and line.", is_snippet: true, starred: false, source: "manual", uses: 5, created_at: iso(2000) },
];
const daily: [string, number][] = [];
for (let i = 0; i < 80; i++) {
  const d = new Date(now - i * 86400000);
  if (i % 7 === 6 && i > 10) continue;
  daily.push([d.toISOString().slice(0, 10), Math.round(150 + 900 * Math.abs(Math.sin(i * 1.7)))]);
}

const models = [
  ["tiny.en", "Tiny (English)", "speech", 78, true, "Fastest, lowest accuracy", false],
  ["base.en", "Base (English)", "speech", 148, true, "Fast on any CPU", false],
  ["small.en", "Small (English)", "speech", 488, true, "Good accuracy, fine on modern CPUs", false],
  ["small", "Small (multilingual)", "speech", 488, false, "Good accuracy, 99 languages", false],
  ["large-v3-turbo-q5", "Large v3 Turbo (quantized)", "speech", 574, false, "Best speed/accuracy with a GPU, 99 languages", true],
  ["large-v3-turbo", "Large v3 Turbo", "speech", 1625, false, "Best accuracy with a GPU, 99 languages", false],
  ["qwen3-1.7b", "Qwen3 1.7B (fast)", "llm", 1107, false, "Light cleanup model for CPU-only machines", false],
  ["qwen3-4b-instruct", "Qwen3 4B Instruct 2507 (recommended)", "llm", 2497, false, "Best local cleanup quality, fast with a GPU", true],
].map(([id, label, kind, size_mb, english_only, note, downloaded]) => ({ id, label, kind, size_mb, english_only, note, downloaded, file: "", url: "" }));

export function installMocks() {
  mockWindows("main");
  mockIPC(
    (cmd, args: any) => {
      switch (cmd) {
        case "get_settings":
          return settings;
        case "save_settings":
          Object.assign(settings, args.settings);
          return null;
        case "set_microphone":
          settings.microphone = args.microphone;
          return null;
        case "history_list":
          return history.filter((h) => !args.query || h.text.toLowerCase().includes(args.query.toLowerCase()));
        case "stats":
          return { total_words: 18432, total_dictations: 1204, words_this_week: 3120, avg_wpm: 142, streak_days: 12, minutes_saved: 341, top_apps: [["Slack", 6120], ["Google Chrome", 4810], ["Outlook", 3302], ["Cursor", 2210], ["Notepad", 1190], ["WhatsApp", 800]], daily };
        case "dict_list":
          return args.snippets ? snippets : dict;
        case "list_models":
          return models;
        case "hardware":
          return { nvidia: true, threads: 32, platform: "windows", speech_engine: "Speech engine (NVIDIA GPU)", cleanup_engine: "Cleanup engine (GPU, Vulkan)", speech_engine_ready: true, cleanup_engine_ready: true, whisper_running: true, llama_running: true, data_dir: "" };
        case "list_mics":
          return ["Microphone (Focusrite USB Audio)", "Headset Microphone (Jabra Evolve2)"];
        case "notes_list":
          return [
            { id: 1, content: "Flow launch checklist\n- Record the demo video\n- Post on the Toolbay community\n- Ask three friends to try hands-free mode", pinned: true, updated_at: iso(30) },
            { id: 2, content: "Ideas for the next free tool: a local meeting notetaker that never uploads audio.", pinned: false, updated_at: iso(600) },
          ];
        case "auto_info":
          return { hardware: { gpu_vendor: "nvidia", gpu_name: "NVIDIA GeForce RTX 4070 Ti", vram_mb: 12000, ram_mb: 65000, threads: 32 }, choice: { tier: "fast-gpu", speech_model: "large-v3-turbo-q5", llm_model: "qwen3-4b-instruct", cleanup: "medium", why: "NVIDIA GeForce RTX 4070 Ti with 11 GB" }, mode: "auto", speech_in_use: "large-v3-turbo-q5" };
        case "app_info":
          return { version: "0.1.0", fromToolbay: false, os: "windows" };
        case "send_feedback":
          return "https://toolbay.ai/community/demo";
        case "sync_status":
        case "sync_now":
          return { state: "ok", account: { name: "Orion", handle: "orion" }, synced_at: iso(2), pending: 0, message: "" };
        case "sync_signed_in":
          return true;
        case "default_shortcuts":
          return settings.shortcuts;
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
