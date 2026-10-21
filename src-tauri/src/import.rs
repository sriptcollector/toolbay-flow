//! Importing dictionary words and snippets from a file, with the same rules as
//! the original app (docs/parity/pages.md): dictionary files are CSV with one
//! word per row and an optional second column for its replacement; snippet files
//! are a JSON list of {"name", "text"}. Rows that don't fit are skipped, never fatal.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Longest dictionary word or snippet name.
pub const MAX_WORD: usize = 60;
/// Longest replacement or snippet text.
pub const MAX_TEXT: usize = 4000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportItem {
    pub phrase: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportPreview {
    /// New items, ready to add.
    pub items: Vec<ImportItem>,
    /// Already in the dictionary (or snippets), left out.
    pub duplicates: usize,
    /// Rows that couldn't be read (too many columns, too long, repeated in the file).
    pub skipped: usize,
    /// Set when nothing in the file could be used.
    pub error: Option<String>,
}

/// One CSV line into cells: commas separate, double quotes group, "" is a quote.
/// None when a quote is left open.
fn cells(line: &str) -> Option<Vec<String>> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.is_empty() {
            quoted = true;
        } else if c == ',' {
            out.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if quoted {
        return None;
    }
    out.push(cur.trim().to_string());
    Some(out)
}

fn finish(found: Vec<ImportItem>, skipped: usize, existing: &[String], what: &str) -> ImportPreview {
    let have: HashSet<String> = existing.iter().map(|s| s.to_lowercase()).collect();
    let (dups, items): (Vec<_>, Vec<_>) = found.into_iter().partition(|i| have.contains(&i.phrase.to_lowercase()));
    let error = (items.is_empty() && dups.is_empty()).then(|| format!("No {what} found in this file"));
    ImportPreview { items, duplicates: dups.len(), skipped, error }
}

/// Dictionary CSV, one entry per line: `word`, or `misheard,word` for a
/// replacement rule (first column what is heard, second how to write it).
pub fn parse_dictionary_csv(text: &str, existing: &[String]) -> ImportPreview {
    let mut seen = HashSet::new();
    let mut found = vec![];
    let mut skipped = 0;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some(c) = cells(line) else {
            skipped += 1;
            continue;
        };
        let c: Vec<String> = c.into_iter().filter(|s| !s.is_empty()).collect();
        if c.is_empty() || c.len() > 2 || c[0].chars().count() > MAX_WORD || c.get(1).map(|r| r.chars().count() > MAX_TEXT).unwrap_or(false) {
            skipped += 1;
            continue;
        }
        if !seen.insert(c[0].to_lowercase()) {
            skipped += 1;
            continue;
        }
        found.push(ImportItem { phrase: c[0].clone(), replacement: c.get(1).cloned() });
    }
    finish(found, skipped, existing, "dictionary words")
}

/// Snippets JSON: `[{"name": "my email", "text": "me@example.com"}, ...]`.
pub fn parse_snippets_json(text: &str, existing: &[String]) -> ImportPreview {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return ImportPreview { error: Some("This file isn't valid JSON".into()), ..Default::default() };
    };
    let Some(arr) = v.as_array() else {
        return ImportPreview { error: Some("The file should hold a list of snippets".into()), ..Default::default() };
    };
    let mut seen = HashSet::new();
    let mut found = vec![];
    let mut skipped = 0;
    for it in arr {
        let name = it.get("name").and_then(|x| x.as_str()).map(str::trim).unwrap_or("");
        let body = it.get("text").and_then(|x| x.as_str()).map(str::trim).unwrap_or("");
        if name.is_empty() || body.is_empty() || name.chars().count() > MAX_WORD || body.chars().count() > MAX_TEXT || !seen.insert(name.to_lowercase()) {
            skipped += 1;
            continue;
        }
        found.push(ImportItem { phrase: name.into(), replacement: Some(body.into()) });
    }
    finish(found, skipped, existing, "snippets")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_csv() {
        let csv = "Toolbay\n\n\"tool bay\",Toolbay\nKubernetes, k8s ,extra\ntoolbay\n\"open quote\nSiobhan\n";
        let p = parse_dictionary_csv(csv, &["siobhan".into()]);
        assert_eq!(
            p.items,
            vec![
                ImportItem { phrase: "Toolbay".into(), replacement: None },
                ImportItem { phrase: "tool bay".into(), replacement: Some("Toolbay".into()) },
            ]
        );
        assert_eq!(p.duplicates, 1); // Siobhan is already there
        assert_eq!(p.skipped, 3); // three columns, repeated in file, open quote
        assert!(p.error.is_none());
        assert_eq!(parse_dictionary_csv(&"x".repeat(61), &[]).skipped, 1);
        assert!(parse_dictionary_csv("", &[]).error.is_some());
        assert_eq!(cells(r#""say ""hi""",b"#).unwrap(), vec!["say \"hi\"", "b"]);
    }

    #[test]
    fn snippets_json() {
        let j = r#"[{"name":"my email","text":"me@example.com"},{"name":"","text":"x"},{"name":"My Email","text":"dup"},{"text":"no name"}]"#;
        let p = parse_snippets_json(j, &[]);
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.skipped, 3);
        assert!(parse_snippets_json("{}", &[]).error.is_some());
        assert!(parse_snippets_json("not json", &[]).error.is_some());
    }
}
