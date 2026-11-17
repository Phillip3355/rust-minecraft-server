//! Lightweight scanners for Rust source text (no syn): enough to pull match-arm
//! string patterns and serde rename/alias values out of Pumpkin's loaders and
//! codegen, so the audit follows the pinned source instead of a hand-copied list.

/// Index just past the string literal (normal or raw) starting at `i`.
fn skip_string(b: &[u8], i: usize) -> usize {
    if b[i] == b'r' {
        let mut j = i + 1;
        let mut hashes = 0;
        while j < b.len() && b[j] == b'#' {
            hashes += 1;
            j += 1;
        }
        j += 1; // opening quote
        while j < b.len() {
            if b[j] == b'"' && b[j + 1..].iter().take(hashes).filter(|&&c| c == b'#').count() == hashes {
                return j + 1 + hashes;
            }
            j += 1;
        }
        return j;
    }
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    j
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The brace-balanced block whose `{` is the first one at/after `start`.
pub fn block_after(text: &str, start: usize) -> Option<&str> {
    let b = text.as_bytes();
    let open = start + text[start..].find('{')?;
    let mut depth = 0i32;
    let mut j = open;
    while j < b.len() {
        let c = b[j];
        if c == b'"' {
            j = skip_string(b, j);
            continue;
        }
        if c == b'r' && j > 0 && !is_ident(b[j - 1]) {
            let mut k = j + 1;
            while k < b.len() && b[k] == b'#' {
                k += 1;
            }
            if k < b.len() && b[k] == b'"' {
                j = skip_string(b, j);
                continue;
            }
        }
        if c == b'\'' {
            if b.get(j + 1) == Some(&b'\\') {
                if let Some(off) = text[j + 3..].find('\'') {
                    j = j + 3 + off + 1;
                    continue;
                }
            } else if b.get(j + 2) == Some(&b'\'') {
                j += 3;
                continue;
            }
        }
        if c == b'/' && b.get(j + 1) == Some(&b'/') {
            j = text[j..].find('\n').map_or(b.len(), |o| j + o);
            continue;
        }
        if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(&text[open..=j]);
            }
        }
        j += 1;
    }
    None
}

fn find_item<'a>(text: &'a str, kw: &str, name: &str) -> Option<&'a str> {
    let pat = format!("{kw} {name}");
    let mut from = 0;
    while let Some(off) = text[from..].find(&pat) {
        let at = from + off;
        let end = at + pat.len();
        let before_ok = at == 0 || !is_ident(text.as_bytes()[at - 1]);
        let after_ok = text.as_bytes().get(end).is_none_or(|&c| !is_ident(c));
        if before_ok && after_ok {
            return block_after(text, end);
        }
        from = end;
    }
    None
}

pub fn fn_body<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    find_item(text, "fn", name)
}

pub fn enum_body<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    find_item(text, "enum", name)
}

/// All `"..."` literal contents in a line fragment (escapes kept verbatim).
pub fn string_literals(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let end = skip_string(b, i);
            out.push(s[i + 1..end.saturating_sub(1).max(i + 1)].to_string());
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// String literals that appear as match-arm patterns (left of `=>`), including
/// patterns split over several `| "..."` lines.
pub fn match_arm_literals(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for raw in body.lines() {
        let line = raw.trim();
        let starts_pattern = line.starts_with('"') || (line.starts_with('|') && (line[1..].trim_start().starts_with('"') || !buf.is_empty()));
        if starts_pattern {
            if let Some((lhs, _)) = line.split_once("=>") {
                buf.push(' ');
                buf.push_str(lhs);
                out.extend(string_literals(&buf));
                buf.clear();
            } else {
                buf.push(' ');
                buf.push_str(line);
            }
        } else {
            buf.clear();
        }
    }
    out
}

/// `rename = "..."`, `alias = "..."` and `rename(deserialize = "...")` values.
pub fn serde_names(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for kw in ["rename", "alias"] {
        let mut from = 0;
        while let Some(off) = body[from..].find(kw) {
            let at = from + off;
            from = at + kw.len();
            let rest = body[from..].trim_start();
            let rest = rest.strip_prefix('(').map_or(rest, |r| r.trim_start().strip_prefix("deserialize").unwrap_or(r).trim_start());
            if let Some(r) = rest.strip_prefix('=') {
                if let Some(lit) = string_literals(r.trim_start().split('\n').next().unwrap_or("")).into_iter().next() {
                    out.push(lit);
                }
            }
        }
    }
    out
}

/// First string literal on every line containing `marker`.
pub fn first_literal_on_lines(text: &str, marker: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains(marker))
        .filter_map(|l| string_literals(l).into_iter().next())
        .collect()
}

/// Literal following each occurrence of `prefix` (e.g. `registry_id : "`).
pub fn literals_after(text: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(off) = text[from..].find(prefix) {
        let at = from + off + prefix.len();
        if let Some(end) = text[at..].find('"') {
            out.push(text[at..at + end].to_string());
        }
        from = at;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r##"
fn other() { match x { "nope" => 1 } }
fn parse(v: &str) -> u8 {
    let s = "{ not a brace";
    let c = '{';
    let r = r#"}"#;
    match v {
        "constant" => 1,
        "cache" | "interpolated" | "flat_cache"
        | "cache_once" => {
            match inner {
                "floor" => 2,
                _ => 3,
            }
        }
        other => panic!("unknown {other}"),
    }
}
#[derive(Deserialize)]
#[serde(tag = "processor_type")]
enum Raw {
    #[serde(rename = "minecraft:rule", alias = "rule")]
    Rule,
    #[serde(
        rename = "minecraft:capped",
        alias = "capped"
    )]
    Capped,
}
"##;

    #[test]
    fn extracts_arms_from_named_fn_only() {
        let body = fn_body(SRC, "parse").unwrap();
        let arms = match_arm_literals(body);
        assert_eq!(arms, ["constant", "cache", "interpolated", "flat_cache", "cache_once", "floor"]);
        assert!(!arms.contains(&"nope".to_string()));
    }

    #[test]
    fn extracts_serde_names() {
        let body = enum_body(SRC, "Raw").unwrap();
        let mut n = serde_names(body);
        n.sort();
        assert_eq!(n, ["capped", "minecraft:capped", "minecraft:rule", "rule"]);
    }
}
