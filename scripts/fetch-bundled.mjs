// Downloads what ships inside the installer so Flow works the moment it's installed:
// a CPU speech engine (whisper.cpp) and a small starter speech model.
// Better models are fetched by the app in the background, chosen for the computer.
// Runs before every `tauri build` (see package.json). Skips files already present.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, renameSync, rmSync, statSync, writeFileSync, copyFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dirname, "..", "src-tauri", "bundled");
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || "";
const os = triple.includes("windows") ? "windows" : triple.includes("apple") ? "macos" : process.platform === "win32" ? "windows" : process.platform === "darwin" ? "macos" : "linux";
const arch = triple ? (triple.startsWith("aarch64") ? "arm64" : "x86_64") : process.arch === "arm64" ? "arm64" : "x86_64";

const MODEL = {
  file: "ggml-base.en-q5_1.bin",
  url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin",
  sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
};

const ENGINE = {
  windows: { id: "whisper-cpu", url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip" },
  macos: {
    id: arch === "arm64" ? "whisper-metal" : "whisper-mac-intel",
    url: `https://github.com/sriptcollector/toolbay-flow/releases/download/engines-v1/whisper-server-macos-${arch}.tar.gz`,
  },
}[os];

async function download(url, dest) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  writeFileSync(dest + ".part", Buffer.from(await res.arrayBuffer()));
  renameSync(dest + ".part", dest);
}

function sha256(file) {
  return createHash("sha256").update(require_fs().readFileSync(file)).digest("hex");
}
function require_fs() {
  return globalThis.__fs;
}

async function main() {
  globalThis.__fs = await import("node:fs");
  mkdirSync(join(root, "models"), { recursive: true });
  const model = join(root, "models", MODEL.file);
  if (!existsSync(model)) {
    console.log(`bundle: downloading ${MODEL.file}`);
    await download(MODEL.url, model);
  }
  if (sha256(model) !== MODEL.sha256) {
    rmSync(model);
    throw new Error("starter model checksum mismatch");
  }
  if (!ENGINE) {
    console.log("bundle: no engine for this platform");
    return;
  }
  const dir = join(root, "engines", ENGINE.id);
  const exe = join(dir, os === "windows" ? "whisper-server.exe" : "whisper-server");
  if (!existsSync(exe)) {
    const tmp = join(root, "tmp");
    rmSync(tmp, { recursive: true, force: true });
    mkdirSync(tmp, { recursive: true });
    const pkg = join(tmp, os === "windows" ? "pkg.zip" : "pkg.tar.gz");
    console.log(`bundle: downloading ${ENGINE.url}`);
    await download(ENGINE.url, pkg);
    // Windows' own bsdtar reads zip files; Git Bash's GNU tar does not.
    const tar = process.platform === "win32" ? join(process.env.SystemRoot || "C:\\Windows", "System32", "tar.exe") : "tar";
    execFileSync(tar, ["-xf", pkg, "-C", tmp]);
    mkdirSync(dir, { recursive: true });
    // Keep only the server and the libraries it needs.
    const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]));
    for (const f of walk(tmp)) {
      const name = f.split(/[\\/]/).pop();
      const keep = name === "whisper-server" || name === "whisper-server.exe" || (name.endsWith(".dll") && !/^(SDL2|parakeet|llama)\.dll$/i.test(name)) || name.endsWith(".dylib");
      if (keep) copyFileSync(f, join(dir, name));
    }
    rmSync(tmp, { recursive: true, force: true });
    if (os !== "windows") execFileSync("chmod", ["+x", exe]);
  }
  console.log(`bundle: ready (${os}-${arch})`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
