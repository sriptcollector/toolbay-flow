// Dropped-words regression test. Builds hard recordings from text-to-speech
// (quiet voice, soft start, long pauses, trailing words), runs them through Flow's
// streaming + silence-trimming path (`toolbay-flow --pipeline`) and fails if words
// go missing compared with a plain whole-file transcription.
// Usage: node eval/pipeline.mjs <toolbay-flow.exe (debug build)> <whisper-url>
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [exe, whisper] = process.argv.slice(2);
const dir = join(import.meta.dirname, "audio", "pipeline");
mkdirSync(dir, { recursive: true });

const say = {
  long: "People should be allowed to attach their GitHub, and then it will automatically create a social profile that shows everything they built. Public projects show by default, but private ones can show too if they choose that during onboarding. It should feel like a social network for builders.",
  pause: "First I want to talk about the installer.",
  pause2: "And then, after thinking about it, the settings page needs a search box.",
  trail: "Please send the final numbers to the team before Friday afternoon",
};

function tts(id, text) {
  const f = join(dir, `${id}.raw.wav`);
  if (!existsSync(f)) {
    const ps = `Add-Type -AssemblyName System.Speech; $s=New-Object System.Speech.Synthesis.SpeechSynthesizer; $s.Rate=-1; $f=New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000,[System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,[System.Speech.AudioFormat.AudioChannel]::Mono); $s.SetOutputToWaveFile('${f}',$f); $s.Speak('${text.replace(/'/g, "''")}'); $s.Dispose()`;
    execFileSync("powershell", ["-NoProfile", "-Command", ps]);
  }
  const b = readFileSync(f);
  const n = (b.length - 44) / 2;
  const s = new Float32Array(n);
  for (let i = 0; i < n; i++) s[i] = b.readInt16LE(44 + 2 * i) / 32768;
  return s;
}

function write(id, s) {
  const buf = Buffer.alloc(44 + s.length * 2);
  buf.write("RIFF", 0);
  buf.writeUInt32LE(36 + s.length * 2, 4);
  buf.write("WAVEfmt ", 8);
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20);
  buf.writeUInt16LE(1, 22);
  buf.writeUInt32LE(16000, 24);
  buf.writeUInt32LE(32000, 28);
  buf.writeUInt16LE(2, 32);
  buf.writeUInt16LE(16, 34);
  buf.write("data", 36);
  buf.writeUInt32LE(s.length * 2, 40);
  // Quiet room noise so "silence" is realistic.
  for (let i = 0; i < s.length; i++) buf.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round((s[i] + (Math.random() - 0.5) * 0.0016) * 32767))), 44 + 2 * i);
  const p = join(dir, `${id}.wav`);
  writeFileSync(p, buf);
  return p;
}

const scale = (s, g) => s.map((x) => x * g);
const silence = (sec) => new Float32Array(Math.round(sec * 16000));
const cat = (...parts) => {
  const out = new Float32Array(parts.reduce((a, p) => a + p.length, 0));
  let o = 0;
  for (const p of parts) out.set(p, o), (o += p.length);
  return out;
};

const cases = {
  // A soft voice far from the mic: peaks around 0.04 (the old fixed threshold cut this).
  quiet: write("quiet", scale(tts("long", say.long), 0.12)),
  // Fades in over the first second, like someone starting to talk as they press.
  "soft-start": write("soft-start", tts("long", say.long).map((x, i) => x * Math.min(1, i / 16000) * 0.5)),
  // A 3-second thinking pause in the middle.
  "long-pause": write("long-pause", cat(scale(tts("pause", say.pause), 0.4), silence(3), scale(tts("pause2", say.pause2), 0.4))),
  // The last words trail off quietly.
  trailing: write("trailing", (() => { const s = tts("trail", say.trail); return s.map((x, i) => x * (i > s.length * 0.6 ? 0.08 : 0.5)); })()),
};

const out = execFileSync(exe, ["--pipeline", whisper, ...Object.values(cases)], { encoding: "utf8" });
let fail = 0;
out.trim().split("\n").forEach((l, i) => {
  const r = JSON.parse(l);
  const id = Object.keys(cases)[i];
  const ok = r.recall >= 0.9;
  if (!ok) fail++;
  console.log(`${ok ? "PASS" : "FAIL"} ${id}: recall ${r.recall} (${r.chunks} streamed chunks)${ok ? "" : `\n     got:   ${r.streamed}\n     whole: ${r.whole}`}`);
});
console.log(fail ? `\n${fail} failed` : "\nall passed");
process.exit(fail ? 1 : 0);
