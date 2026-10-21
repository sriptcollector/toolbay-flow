// Dictation quality eval: speaks each case with Windows text-to-speech, runs it
// through Flow's real speech + cleanup path (`toolbay-flow --selftest`) and checks
// the result. Usage: node eval/run.mjs <toolbay-flow.exe> <whisper-url> <llama-url>
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [exe, whisper, llama] = process.argv.slice(2);
const dir = join(import.meta.dirname, "audio");
mkdirSync(dir, { recursive: true });
const cases = JSON.parse(readFileSync(join(import.meta.dirname, "cases.json"), "utf8"));

// Synthesize missing clips (16 kHz mono WAV) with System.Speech.
const todo = cases.filter((c) => !existsSync(join(dir, `${c.id}.wav`)));
if (todo.length) {
  const ps = [
    "Add-Type -AssemblyName System.Speech",
    "$s = New-Object System.Speech.Synthesis.SpeechSynthesizer",
    "$f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)",
    ...todo.map((c) => `$s.SetOutputToWaveFile('${join(dir, c.id + ".wav")}', $f); $s.Speak('${c.say.replace(/'/g, "''")}')`),
    "$s.Dispose()",
  ].join("; ");
  execFileSync("powershell", ["-NoProfile", "-Command", ps]);
}

const args = ["--selftest", whisper, llama, ...cases.map((c) => `${join(dir, c.id + ".wav")}@${c.app}`)];
const out = execFileSync(exe, args, { encoding: "utf8", maxBuffer: 1 << 24 });
const results = out.trim().split("\n").map((l) => JSON.parse(l));

let pass = 0;
const report = [];
cases.forEach((c, i) => {
  const r = results[i];
  const t = r.text;
  const fails = [];
  for (const h of c.has ?? []) if (!t.includes(h)) fails.push(`missing "${h}"`);
  for (const n of c.not ?? []) if (t.toLowerCase().includes(n.toLowerCase())) fails.push(`contains "${n}"`);
  for (const re of c.re ?? []) if (!new RegExp(re, "m").test(t)) fails.push(`no match /${re}/`);
  if (c.maxWords && t.split(/\s+/).length > c.maxWords) fails.push(`too long (${t.split(/\s+/).length} words)`);
  if (c.minSentences && (t.match(/[.!?](\s|$)/g) ?? []).length < c.minSentences) fails.push("not split into sentences");
  if (!fails.length) pass++;
  report.push({ id: c.id, ok: !fails.length, fails, heard: r.raw, text: t, ms: r.speech_ms + r.cleanup_ms });
});
for (const r of report) console.log(`${r.ok ? "PASS" : "FAIL"} ${r.id} (${r.ms} ms)${r.ok ? "" : `\n     heard: ${JSON.stringify(r.heard)}\n     got:   ${JSON.stringify(r.text)}\n     why:   ${r.fails.join("; ")}`}`);
console.log(`\n${pass}/${cases.length} passed`);
writeFileSync(join(import.meta.dirname, "last-report.json"), JSON.stringify(report, null, 2));
