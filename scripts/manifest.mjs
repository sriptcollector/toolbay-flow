// Writes toolbay.json (the Toolbay hub's install manifest) with the real size and
// sha256 of each release asset. Usage: node scripts/manifest.mjs <assets-dir> [out]
// The installer bundles a starter speech model, so no "files" entries are needed:
// one install and dictation works; Flow upgrades its models in the background.
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2] || ".";
const out = process.argv[3] || "toolbay.json";
const pkg = JSON.parse(readFileSync(join(import.meta.dirname, "..", "package.json"), "utf8"));
const v = pkg.version;
const brand = JSON.parse(readFileSync(join(import.meta.dirname, "..", "brand.json"), "utf8"));

function entry(asset, extra) {
  const p = join(dir, asset);
  const e = { asset, ...extra };
  if (existsSync(p)) {
    const buf = readFileSync(p);
    e.size = statSync(p).size;
    e.sha256 = createHash("sha256").update(buf).digest("hex");
  }
  return e;
}

const manifest = {
  schema: 1,
  id: "flow",
  name: "Toolbay Flow",
  tagline: "Wispr Flow, reverse engineered and free",
  description:
    "We reverse engineered Wispr Flow and made it free. Stop paying for dumb shit. Hold a key, speak, let go: clean, punctuated text appears wherever your cursor is. Speech recognition and cleanup run on your computer. No account, no word limits, no telemetry.",
  version: v,
  repo: "sriptcollector/toolbay-flow",
  identifier: "ai.toolbay.flow",
  license: "MIT",
  homepage: "https://github.com/sriptcollector/toolbay-flow",
  icon: "https://raw.githubusercontent.com/sriptcollector/toolbay-flow/main/app-icon.png",
  replaces: brand.inspiredBy.name,
  inspiredBy: brand.inspiredBy,
  categories: ["productivity", "voice", "ai"],
  platforms: {
    "windows-x86_64": entry(`Toolbay.Flow_${v}_x64-setup.exe`, {
      kind: "nsis",
      // /NS: the hub makes its own desktop shortcut (opens Flow through the hub).
      silentArgs: ["/S", "/NS"],
      exe: "toolbay-flow.exe",
      uninstaller: "uninstall.exe",
    }),
    "darwin-aarch64": entry(`Toolbay.Flow_${v}_aarch64.dmg`, { kind: "dmg", app: "Toolbay Flow.app" }),
    "darwin-x86_64": entry(`Toolbay.Flow_${v}_x64.dmg`, { kind: "dmg", app: "Toolbay Flow.app" }),
  },
  files: [],
  launch: { args: ["--from-toolbay"], singleInstance: true, control: { show: ["--show"], hide: ["--hide"], quit: ["--quit"] }, stateFile: "running.json" },
};

writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`wrote ${out} for ${v}`);
