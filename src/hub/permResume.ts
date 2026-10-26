/**
 * macOS often only applies a newly allowed Accessibility / Input Monitoring permission after the
 * app restarts. When someone clicks Allow we remember where they were, so after the restart
 * (ours or macOS's own "Quit & Reopen") Flow brings them right back there and shows whether it worked.
 */
const KEY = "permResume";
const FRESH_MS = 20 * 60 * 1000;

export type ResumeWhere = "setup" | "app";

export function markPermissionAsked(where: ResumeWhere) {
  localStorage.setItem(KEY, JSON.stringify({ where, at: Date.now(), pid: launchId }));
}

/** Where the person was when they clicked Allow in an earlier run of Flow, if it was recent. */
export function resumedFrom(): ResumeWhere | null {
  try {
    const r = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!r || Date.now() - r.at > FRESH_MS || r.pid === launchId) return null;
    return r.where;
  } catch {
    return null;
  }
}

/** Whether Allow was clicked during this run (so a restart is what's missing). */
export function askedThisRun(): boolean {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "null")?.pid === launchId;
  } catch {
    return false;
  }
}

export function clearPermissionResume() {
  localStorage.removeItem(KEY);
}

// Different every time Flow starts; tells "this run" from "the run before the restart".
const launchId = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
