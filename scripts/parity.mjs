// Parity coverage: share of docs/FEATURES.md rows whose behaviour has been checked
// against the real app (traced with the REA tools and compared with ours), marked
// "Parity: matched" in the row. Omitted rows don't count.
// Usage: node scripts/parity.mjs  ->  "4% (2 of 49 features checked against Wispr Flow)"
import { readFileSync } from "node:fs";
import { join } from "node:path";

const rows = readFileSync(join(import.meta.dirname, "..", "docs", "FEATURES.md"), "utf8")
  .split("\n")
  .filter((l) => l.startsWith("| ") && !/^\| (Feature|Original screen)/.test(l))
  .filter((l) => !/\|\s*Omitted/.test(l));
const matched = rows.filter((l) => /Parity: matched/.test(l)).length;
const pct = rows.length ? Math.round((100 * matched) / rows.length) : 0;
console.log(`${pct}% (${matched} of ${rows.length} features checked against Wispr Flow)`);
