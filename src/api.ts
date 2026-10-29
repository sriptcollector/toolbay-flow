import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export type Action =
  | "push_to_talk"
  | "hands_free"
  | "command"
  | "paste_last"
  | "copy_last"
  | "dismiss"
  | "transform1"
  | "transform2"
  | "transform3";
export type Style = "formal" | "casual" | "very_casual" | "excited";
export type CleanupLevel = "none" | "light" | "medium" | "high";
export type DataPolicy = "store_normally" | "delete_after24h" | "never_store";
export type Provider = "local" | "open_ai_compatible" | "anthropic";

export interface Shortcut {
  action: Action;
  keys: number[];
}
export interface CloudLlm {
  base_url: string;
  api_key: string;
  model: string;
}
export interface Transform {
  id: string;
  name: string;
  instructions: string;
}
export interface Settings {
  onboarded: boolean;
  show_bar: boolean;
  sounds: boolean;
  microphone: string | null;
  language: string;
  speech_model: string;
  use_gpu: boolean;
  cleanup_level: CleanupLevel;
  /** Picked by the person: Auto never changes it. */
  cleanup_chosen: boolean;
  llm_provider: Provider;
  local_llm_model: string;
  cloud_llm: CloudLlm;
  anthropic: CloudLlm;
  styles: { personal: Style; work: Style; email: Style; other: Style };
  context_awareness: boolean;
  auto_learn_words: boolean;
  command_mode: boolean;
  data_policy: DataPolicy;
  keep_audio: boolean;
  shortcuts: Shortcut[];
  transforms: Transform[];
  app_categories: Record<string, string>;
  restore_clipboard: boolean;
  name: string;
  model_mode: "auto" | "manual";
  hw_fingerprint: string;
  auto_reason: string;
  auto_slow_speech: boolean;
  auto_slow_cleanup: boolean;
  desktop_shortcut: boolean;
  profile: Record<string, string[]> | null;
  press_enter_command: boolean;
  bar_snooze_until: number;
  debug_recordings: boolean;
  appearance: "light" | "dark" | "system";
  sync_enabled: boolean;
  sync_asked: boolean;
  sync_history: boolean;
  engine: "local" | "cloud";
  cloud_asr: CloudLlm;
}
export interface AutoInfo {
  hardware: { gpu_vendor: string; gpu_name: string; vram_mb: number; ram_mb: number; threads: number };
  choice: { tier: string; speech_model: string; llm_model: string; cleanup: CleanupLevel; why: string };
  mode: string;
  speech_in_use: string | null;
}
export interface ModelRow {
  id: string;
  label: string;
  kind: "speech" | "llm";
  file: string;
  url: string;
  size_mb: number;
  english_only: boolean;
  note: string;
  downloaded: boolean;
}
export interface Hardware {
  nvidia: boolean;
  threads: number;
  platform: string;
  speech_engine: string | null;
  cleanup_engine: string | null;
  speech_engine_ready: boolean;
  cleanup_engine_ready: boolean;
  whisper_running: boolean;
  llama_running: boolean;
  data_dir: string;
}
export interface HistoryItem {
  id: number;
  created_at: string;
  mode: string;
  raw_text: string;
  text: string;
  app: string;
  window_title: string;
  url: string;
  language: string;
  duration_ms: number;
  latency_ms: number;
  words: number;
  status: string;
  has_audio: boolean;
  edited_text: string | null;
}
export interface ImportItem {
  phrase: string;
  replacement: string | null;
}
export interface ImportPreview {
  items: ImportItem[];
  duplicates: number;
  skipped: number;
  error: string | null;
}
export interface DictEntry {
  id: number;
  phrase: string;
  replacement: string | null;
  is_snippet: boolean;
  starred: boolean;
  source: string;
  uses: number;
  created_at: string;
}
export interface Note {
  id: number;
  content: string;
  pinned: boolean;
  updated_at: string;
}
export interface Stats {
  total_words: number;
  total_dictations: number;
  words_this_week: number;
  avg_wpm: number;
  streak_days: number;
  minutes_saved: number;
  top_apps: [string, number][];
  daily: [string, number][];
}
export interface Progress {
  id: string;
  done: number;
  total: number;
  stage: string;
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  /** Changes only the microphone, so a stale copy of the settings never overwrites the rest. */
  setMicrophone: (microphone: string | null) => invoke<void>("set_microphone", { microphone }),
  defaultShortcuts: () => invoke<Shortcut[]>("default_shortcuts"),
  listModels: () => invoke<ModelRow[]>("list_models"),
  hardware: () => invoke<Hardware>("hardware"),
  autoInfo: () => invoke<AutoInfo>("auto_info"),
  rerunAuto: () => invoke<void>("rerun_auto"),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  downloadEngines: (kinds: string[]) => invoke<void>("download_engines", { kinds }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),
  startEngines: () => invoke<void>("start_engines"),
  listMics: () => invoke<string[]>("list_mics"),
  micTest: (on: boolean) => invoke<void>("mic_test", { on }),
  historyList: (query: string, limit = 100, offset = 0) => invoke<HistoryItem[]>("history_list", { query, limit, offset }),
  historyDelete: (id: number) => invoke<void>("history_delete", { id }),
  historyClear: () => invoke<void>("history_clear"),
  historyRetry: (id: number) => invoke<string>("history_retry", { id }),
  historyAudio: (id: number) => invoke<number[]>("history_audio", { id }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  dictList: (snippets: boolean) => invoke<DictEntry[]>("dict_list", { snippets }),
  dictSave: (entry: Partial<DictEntry>) =>
    invoke<number>("dict_save", {
      entry: { id: 0, replacement: null, is_snippet: false, starred: false, source: "manual", uses: 0, created_at: "", phrase: "", ...entry },
    }),
  dictDelete: (id: number) => invoke<void>("dict_delete", { id }),
  dictImportPreview: (text: string, snippets: boolean) => invoke<ImportPreview>("dict_import_preview", { text, snippets }),
  dictImport: (items: ImportItem[], snippets: boolean) => invoke<number>("dict_import", { items, snippets }),
  undoLearned: () => invoke<void>("undo_learned"),
  notesList: () => invoke<Note[]>("notes_list"),
  noteSave: (note: Partial<Note>) => invoke<number>("note_save", { note: { id: 0, content: "", pinned: false, updated_at: "", ...note } }),
  noteDelete: (id: number) => invoke<void>("note_delete", { id }),
  stats: () => invoke<Stats>("stats"),
  recordShortcut: (on: boolean) => invoke<void>("record_shortcut", { on }),
  previewCleanup: (text: string, appName: string) => invoke<string>("preview_cleanup", { text, appName }),
  barAction: (action: string) => invoke<void>("bar_action", { action }),
  barHit: (rects: number[][]) => invoke<void>("bar_hit", { rects }),
  openHub: (page?: string) => invoke<void>("open_hub", { page }),
  hotkeysEnabled: (on: boolean) => invoke<void>("hotkeys_enabled", { on }),
  appInfo: () => invoke<{ version: string; fromToolbay: boolean; os: string }>("app_info"),
  openDataDir: () => invoke<void>("open_data_dir"),
  quit: () => invoke<void>("quit"),
  quitFlushed: () => invoke<void>("quit_flushed"),
  uiReady: () => invoke<void>("ui_ready"),
  ownKey: (vk: number, down: boolean) => invoke<void>("own_key", { vk, down }),
  sendFeedback: (kind: string, text: string, debug: boolean, isPublic: boolean) => invoke<string>("send_feedback", { kind, text, debug, public: isPublic }),
  syncStatus: () => invoke<SyncStatus>("sync_status"),
  syncNow: () => invoke<SyncStatus>("sync_now"),
  syncSignedIn: () => invoke<boolean>("sync_signed_in"),
  syncSignIn: () => invoke<void>("sync_sign_in"),
  syncDeleteRemote: () => invoke<void>("sync_delete_remote"),
  toolbayAccount: () => invoke<{ name: string; handle: string; avatar_url: string | null } | null>("toolbay_account"),
  setDesktopShortcut: (on: boolean) => invoke<void>("set_desktop_shortcut", { on }),
  hasDesktopShortcut: () => invoke<boolean>("has_desktop_shortcut"),
  micCheck: () => invoke<{ ok: boolean; message: string }>("mic_check"),
  openSystemSettings: (kind: string) => invoke<void>("open_system_settings", { kind }),
  restartApp: () => invoke<void>("restart_app"),
  /** `fnKey` also reads the 🌐-key setting (it starts a process, so only on window focus). */
  macPermissions: (fnKey = true) => invoke<MacPermissions>("mac_permissions", { fnKey }),
};

export interface MacPermissions {
  /** False when macOS blocks the microphone: recordings would be silent. */
  microphone: boolean;
  /** Input Monitoring: the shortcut listener. Flow never asks for Accessibility. */
  input: boolean;
  /** Null when not read this time. */
  fn_free: boolean | null;
}

export interface SyncStatus {
  state: "off" | "signed_out" | "syncing" | "ok" | "offline" | "error" | "";
  account: { name?: string; handle?: string; email?: string; avatarUrl?: string } | null;
  synced_at: string | null;
  pending: number;
  message: string;
}

export function on<T>(event: string, cb: (payload: T) => void): () => void {
  let un: UnlistenFn | null = null;
  let dead = false;
  listen<T>(event, (e) => cb(e.payload)).then((u) => {
    if (dead) u();
    else un = u;
  });
  return () => {
    dead = true;
    un?.();
  };
}

export const ACTION_LABELS: Record<Action, [string, string]> = {
  push_to_talk: ["Dictate", "Hold to talk, release to paste. Double-tap to keep listening."],
  hands_free: ["Hands-free", "Press once to start, again to stop."],
  command: ["Command mode", "Hold and say what to do with the selected text."],
  paste_last: ["Paste last dictation", "Paste your most recent text again."],
  copy_last: ["Copy last dictation", "Copy your most recent text to the clipboard."],
  dismiss: ["Cancel", "Stop listening and throw the recording away."],
  transform1: ["Transform 1", "Rewrite selected text with your first transform."],
  transform2: ["Transform 2", "Rewrite selected text with your second transform."],
  transform3: ["Transform 3", "Rewrite selected text with your third transform."],
};

export const LANGUAGES: [string, string][] = [
  ["auto", "Auto-detect"], ["en", "English"], ["es", "Spanish"], ["fr", "French"], ["de", "German"],
  ["it", "Italian"], ["pt", "Portuguese"], ["nl", "Dutch"], ["pl", "Polish"], ["ru", "Russian"],
  ["uk", "Ukrainian"], ["tr", "Turkish"], ["ar", "Arabic"], ["hi", "Hindi"], ["ja", "Japanese"],
  ["ko", "Korean"], ["zh", "Chinese"], ["vi", "Vietnamese"], ["id", "Indonesian"], ["sv", "Swedish"],
  ["da", "Danish"], ["no", "Norwegian"], ["fi", "Finnish"], ["cs", "Czech"], ["el", "Greek"],
  ["he", "Hebrew"], ["hu", "Hungarian"], ["ro", "Romanian"], ["th", "Thai"], ["ms", "Malay"],
  ["fa", "Persian"], ["bn", "Bengali"], ["ta", "Tamil"], ["ur", "Urdu"], ["ca", "Catalan"],
];
