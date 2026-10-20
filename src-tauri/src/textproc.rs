//! Deterministic text processing: transcript artifacts, rule-based cleanup,
//! dictionary replacements, snippets, and app categories.

use crate::db::DictEntry;
use crate::settings::Style;
use once_cell::sync::Lazy;
use regex::Regex;

static ARTIFACTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\[(blank_audio|silence|music|inaudible|no speech|sound|noise|applause|laughter)[^\]]*\]|\((music|silence|inaudible|applause|laughs?|laughter|sighs?|coughs?|background noise)[^)]*\)|\*[^*]{1,30}\*")
        .unwrap()
});

/// Phrases whisper tends to invent on near-silent audio.
const HALLUCINATIONS: &[&str] = &[
    "thank you.", "thanks for watching!", "thanks for watching.", "thank you for watching.", "you", "bye.",
    "subtitles by the amara.org community", "please subscribe.", ".", "...",
];

pub fn strip_artifacts(s: &str) -> String {
    let t = ARTIFACTS.replace_all(s, " ");
    let t = collapse_ws(&t);
    if HALLUCINATIONS.contains(&t.to_lowercase().as_str()) {
        return String::new();
    }
    t
}

pub fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&line.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    out.trim().to_string()
}

static FILLERS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(^|[\s,])(u+m+|u+h+|e+r+m+|uhm+|hm+|mhm|ah+)([\s,.!?]|$)").unwrap());
static SPACE_BEFORE_PUNCT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+([,.!?;:])").unwrap());
static DOUBLE_PUNCT: Lazy<Regex> = Lazy::new(|| Regex::new(r"([,.!?;:])[,.;:]+").unwrap());
static SCRATCH: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)[^.!?\n]*[.!?]?\s*\b(scratch that|delete that|never mind that)\b[.,!]?\s*").unwrap());

/// Spoken punctuation and layout commands. Applied in Light mode and as a safety
/// net after the LLM (only when the literal words survived).
const SPOKEN: &[(&str, &str)] = &[
    (r"(?i)[,.]?\s*\bnew paragraph\b[,.]?\s*", "\n\n"),
    (r"(?i)[,.]?\s*\b(new line|newline|next line)\b[,.]?\s*", "\n"),
    (r"(?i)\s*\bfull stop\b", "."),
    (r"(?i)\s*\bperiod\b(\s|$)", ".$1"),
    (r"(?i)\s*\bcomma\b", ","),
    (r"(?i)\s*\bquestion mark\b", "?"),
    (r"(?i)\s*\bexclamation (mark|point)\b", "!"),
    (r"(?i)\s*\bsemicolon\b", ";"),
    (r"(?i)\s*\bcolon\b", ":"),
    (r"(?i)\bopen (paren|parenthesis)\s*", "("),
    (r"(?i)\s*\bclose (paren|parenthesis)\b", ")"),
    (r"(?i)\bopen quote\s*", "\""),
    (r"(?i)\s*\b(close|end) quote\b", "\""),
];

static SPOKEN_RE: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| SPOKEN.iter().map(|(p, r)| (Regex::new(p).unwrap(), *r)).collect());

pub fn apply_spoken_punctuation(s: &str) -> String {
    let mut t = s.to_string();
    for (re, rep) in SPOKEN_RE.iter() {
        t = re.replace_all(&t, *rep).to_string();
    }
    t
}

/// Rule-only cleanup used for the "Light" level and as the LLM fallback.
pub fn rule_cleanup(raw: &str, style: &Style) -> String {
    let mut t = raw.to_string();
    t = SCRATCH.replace_all(&t, "").to_string();
    // Fillers (run twice for adjacent ones).
    for _ in 0..2 {
        t = FILLERS.replace_all(&t, "$1$3").to_string();
    }
    t = dedupe_repeats(&t);
    t = apply_spoken_punctuation(&t);
    t = SPACE_BEFORE_PUNCT.replace_all(&t, "$1").to_string();
    t = DOUBLE_PUNCT.replace_all(&t, "$1").to_string();
    t = t.replace(" ,", ",").replace(",,", ",");
    t = collapse_ws(&t);
    t = t.trim_start_matches([',', ';', ':', ' ']).to_string();
    t = capitalize_sentences(&t);
    apply_style(&t, style)
}

/// Removes immediately repeated words ("the the" -> "the"), keeping line breaks.
pub fn dedupe_repeats(s: &str) -> String {
    s.lines()
        .map(|line| {
            let mut out: Vec<&str> = vec![];
            for w in line.split(' ') {
                if let Some(prev) = out.last() {
                    if !w.is_empty() && prev.eq_ignore_ascii_case(w) && w.chars().all(|c| c.is_alphabetic() || c == '\'') {
                        continue;
                    }
                }
                out.push(w);
            }
            out.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn capitalize_sentences(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut cap = true;
    for c in s.chars() {
        if cap && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            cap = false;
        } else {
            out.push(c);
            if c.is_alphanumeric() {
                cap = false;
            }
        }
        if matches!(c, '.' | '!' | '?' | '\n') {
            cap = true;
        }
    }
    // Standalone "i" -> "I".
    static I_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\bi\b('m|'ve|'d|'ll)?").unwrap());
    I_RE.replace_all(&out, |c: &regex::Captures| format!("I{}", c.get(1).map(|m| m.as_str()).unwrap_or(""))).to_string()
}

/// Final style touches that a small model sometimes misses.
pub fn apply_style(s: &str, style: &Style) -> String {
    let mut t = s.trim().to_string();
    if t.is_empty() {
        return t;
    }
    let multi_line = t.contains('\n');
    match style {
        Style::Formal => {
            if !multi_line && !t.ends_with(['.', '!', '?', ':', ')', '"']) && t.split_whitespace().count() > 2 {
                t.push('.');
            }
        }
        Style::Casual => {
            if !multi_line && t.ends_with('.') && !t.ends_with("...") && t.matches(['.', '!', '?']).count() == 1 {
                t.pop();
            }
        }
        Style::VeryCasual => {
            if !multi_line && t.ends_with('.') && !t.ends_with("...") {
                t.pop();
            }
            let mut cs = t.chars();
            if let Some(f) = cs.next() {
                let second = t.chars().nth(1);
                // Keep acronyms and "I" capitalized.
                if f.is_uppercase() && second.map(|c| c.is_lowercase() || c == ' ' && f != 'I').unwrap_or(false) && f != 'I' {
                    t = f.to_lowercase().collect::<String>() + cs.as_str();
                }
            }
        }
        Style::Excited => {
            if !multi_line && t.ends_with('.') && !t.ends_with("...") {
                t.pop();
                t.push('!');
            } else if !multi_line && !t.ends_with(['!', '?']) {
                t.push('!');
            }
        }
    }
    t
}

fn phrase_regex(phrase: &str) -> Option<Regex> {
    let words: Vec<String> = phrase
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(regex::escape)
        .collect();
    if words.is_empty() {
        return None;
    }
    Regex::new(&format!(r"(?i)\b{}\b", words.join(r"[\s,.\-]*"))).ok()
}

/// Dictionary replacement rules (phrase -> replacement), whole-word, case-insensitive.
/// Returns the new text and the ids that fired.
pub fn apply_replacements(text: &str, dict: &[DictEntry]) -> (String, Vec<i64>) {
    let mut t = text.to_string();
    let mut used = vec![];
    for e in dict.iter().filter(|e| !e.is_snippet) {
        let Some(rep) = e.replacement.as_deref().filter(|r| !r.is_empty()) else { continue };
        if let Some(re) = phrase_regex(&e.phrase) {
            if re.is_match(&t) {
                t = re.replace_all(&t, regex::NoExpand(rep)).to_string();
                used.push(e.id);
            }
        }
    }
    (t, used)
}

/// Normalizes words for snippet trigger comparison.
fn norm(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
}

/// If the whole dictation is a snippet trigger, returns the expansion directly.
pub fn exact_snippet<'a>(raw: &str, dict: &'a [DictEntry]) -> Option<&'a DictEntry> {
    let n = norm(raw);
    dict.iter().filter(|e| e.is_snippet).find(|e| !n.is_empty() && norm(&e.phrase) == n)
}

/// Expands snippet triggers that appear inside text.
pub fn expand_snippets(text: &str, dict: &[DictEntry]) -> (String, Vec<i64>) {
    let mut t = text.to_string();
    let mut used = vec![];
    for e in dict.iter().filter(|e| e.is_snippet) {
        let Some(rep) = e.replacement.as_deref() else { continue };
        if let Some(re) = phrase_regex(&e.phrase) {
            if re.is_match(&t) {
                t = re.replace_all(&t, regex::NoExpand(rep)).to_string();
                used.push(e.id);
            }
        }
    }
    (t, used)
}

/// Category used to choose a style: "personal", "work", "email" or "other".
pub fn app_category(exe: &str, title: &str, url: &str, overrides: &std::collections::BTreeMap<String, String>) -> String {
    let exe_l = exe.to_lowercase();
    if let Some(c) = overrides.get(&exe_l) {
        return c.clone();
    }
    let hay = format!("{} {} {}", exe_l, title.to_lowercase(), url.to_lowercase());
    let has = |keys: &[&str]| keys.iter().any(|k| hay.contains(k));
    if has(&["outlook", "thunderbird", "gmail", "mail.google", "superhuman", "hey.com", "proton mail", "mailspring", "spark", "mail -", "- mail", "inbox"]) {
        "email".into()
    } else if has(&["slack", "teams", "zoom", "linear", "jira", "notion", "asana", "clickup", "google chat"]) {
        "work".into()
    } else if has(&["whatsapp", "telegram", "discord", "signal", "messenger", "messages", "imessage", "instagram", "wechat", "line.exe", "snapchat"]) {
        "personal".into()
    } else {
        "other".into()
    }
}

/// Vocabulary hint for the speech model: starred and most used words first.
pub fn vocabulary_prompt(dict: &[DictEntry], max_chars: usize) -> String {
    let mut words: Vec<&DictEntry> = dict.iter().filter(|e| !e.is_snippet).collect();
    words.sort_by(|a, b| b.starred.cmp(&a.starred).then(b.uses.cmp(&a.uses)));
    let mut out = String::new();
    for e in words {
        let w = e.replacement.as_deref().filter(|r| !r.is_empty()).unwrap_or(&e.phrase);
        if out.len() + w.len() + 2 > max_chars {
            break;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(w);
    }
    out
}

/// Levenshtein distance on chars (for auto-learning corrected words).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Words the user corrected after a paste: (old, new) pairs that look like
/// spelling fixes of names or jargon.
pub fn learned_corrections(pasted: &str, now: &str) -> Vec<(String, String)> {
    let tok = |s: &str| -> Vec<String> {
        s.split(|c: char| c.is_whitespace() || ",.!?;:()\"".contains(c)).filter(|w| w.len() >= 2).map(|w| w.to_string()).collect()
    };
    let p = tok(pasted);
    let n = tok(now);
    let pl: std::collections::HashSet<String> = p.iter().map(|w| w.to_lowercase()).collect();
    let nl: std::collections::HashSet<String> = n.iter().map(|w| w.to_lowercase()).collect();
    let removed: Vec<&String> = p.iter().filter(|w| !nl.contains(&w.to_lowercase())).collect();
    let added: Vec<&String> = n.iter().filter(|w| !pl.contains(&w.to_lowercase())).collect();
    // Too many changes means a rewrite, not a spelling fix.
    if removed.is_empty() || removed.len() > 4 || added.len() > 6 {
        return vec![];
    }
    let mut out = vec![];
    for r in &removed {
        let best = added
            .iter()
            .map(|a| (a, levenshtein(&r.to_lowercase(), &a.to_lowercase())))
            .min_by_key(|(_, d)| *d);
        if let Some((a, d)) = best {
            let maxlen = r.len().max(a.len());
            if d > 0 && (d as f32) / (maxlen as f32) <= 0.45 && a.len() >= 3 && a.chars().any(|c| c.is_alphabetic()) {
                out.push((r.to_string(), a.to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillers_and_punctuation() {
        let t = rule_cleanup("um so I think uh we should go comma right question mark", &Style::Formal);
        assert_eq!(t, "So I think we should go, right?");
    }

    #[test]
    fn new_line() {
        let t = rule_cleanup("hello new line world", &Style::Formal);
        assert_eq!(t, "Hello\nWorld");
    }

    #[test]
    fn repeats() {
        assert_eq!(rule_cleanup("the the cat sat", &Style::Casual), "The cat sat");
    }

    #[test]
    fn artifacts() {
        assert_eq!(strip_artifacts("[BLANK_AUDIO]"), "");
        assert_eq!(strip_artifacts(" Hello (music) there "), "Hello there");
    }

    #[test]
    fn replacements_and_snippets() {
        let d = vec![
            DictEntry { id: 1, phrase: "tool bay".into(), replacement: Some("Toolbay".into()), ..Default::default() },
            DictEntry { id: 2, phrase: "my calendar link".into(), replacement: Some("https://cal.com/me".into()), is_snippet: true, ..Default::default() },
        ];
        let (t, _) = apply_replacements("I love tool bay a lot", &d);
        assert_eq!(t, "I love Toolbay a lot");
        let (t, _) = expand_snippets("Here is my calendar link.", &d);
        assert_eq!(t, "Here is https://cal.com/me.");
        assert!(exact_snippet("My calendar link.", &d).is_some());
    }

    #[test]
    fn styles() {
        assert_eq!(apply_style("Sounds good.", &Style::Casual), "Sounds good");
        assert_eq!(apply_style("Sounds good.", &Style::VeryCasual), "sounds good");
        assert_eq!(apply_style("Sounds good.", &Style::Excited), "Sounds good!");
        assert_eq!(apply_style("See you at noon", &Style::Formal), "See you at noon.");
    }

    #[test]
    fn learning() {
        let c = learned_corrections("Meeting with Kubernetis team", "Meeting with Kubernetes team");
        assert_eq!(c, vec![("Kubernetis".to_string(), "Kubernetes".to_string())]);
        assert!(learned_corrections("hello there", "a completely different long rewritten sentence here now").is_empty());
    }

    #[test]
    fn category() {
        let o = Default::default();
        assert_eq!(app_category("OUTLOOK.EXE", "", "", &o), "email");
        assert_eq!(app_category("chrome.exe", "Inbox (3) - me@gmail.com - Gmail", "", &o), "email");
        assert_eq!(app_category("slack.exe", "", "", &o), "work");
        assert_eq!(app_category("WhatsApp.exe", "", "", &o), "personal");
        assert_eq!(app_category("notepad.exe", "Untitled - Notepad", "", &o), "other");
    }
}
