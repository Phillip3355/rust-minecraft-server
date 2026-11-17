//! Issue collection and (dependency-free) JSON rendering.

use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// Stable machine-readable categories so regression tests can assert on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Io,
    RegionHeader,
    RegionSector,
    Compression,
    Nbt,
    ChunkStructure,
    BlockStates,
    Biomes,
    BlockEntity,
    LevelDat,
    PlayerData,
    ItemStack,
    ChunkLost,
    BlockChanged,
    PlayerLost,
    PlayerChanged,
    ItemDuplication,
    ItemLoss,
    Unsupported,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Io => "io",
            Category::RegionHeader => "region_header",
            Category::RegionSector => "region_sector",
            Category::Compression => "compression",
            Category::Nbt => "nbt",
            Category::ChunkStructure => "chunk_structure",
            Category::BlockStates => "block_states",
            Category::Biomes => "biomes",
            Category::BlockEntity => "block_entity",
            Category::LevelDat => "level_dat",
            Category::PlayerData => "player_data",
            Category::ItemStack => "item_stack",
            Category::ChunkLost => "chunk_lost",
            Category::BlockChanged => "block_changed",
            Category::PlayerLost => "player_lost",
            Category::PlayerChanged => "player_changed",
            Category::ItemDuplication => "item_duplication",
            Category::ItemLoss => "item_loss",
            Category::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub category: Category,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub issues: Vec<Issue>,
    /// Free-form counters (chunks scanned, players scanned, ...).
    pub stats: BTreeMap<String, i64>,
}

impl Report {
    pub fn push(&mut self, severity: Severity, category: Category, location: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue { severity, category, location: location.into(), message: message.into() });
    }
    pub fn error(&mut self, c: Category, loc: impl Into<String>, msg: impl Into<String>) {
        self.push(Severity::Error, c, loc, msg)
    }
    pub fn warn(&mut self, c: Category, loc: impl Into<String>, msg: impl Into<String>) {
        self.push(Severity::Warning, c, loc, msg)
    }
    pub fn info(&mut self, c: Category, loc: impl Into<String>, msg: impl Into<String>) {
        self.push(Severity::Info, c, loc, msg)
    }
    pub fn bump(&mut self, key: &str, n: i64) {
        *self.stats.entry(key.to_string()).or_insert(0) += n;
    }
    pub fn merge(&mut self, other: Report) {
        self.issues.extend(other.issues);
        for (k, v) in other.stats {
            self.bump(&k, v);
        }
    }
    pub fn count(&self, s: Severity) -> usize {
        self.issues.iter().filter(|i| i.severity == s).count()
    }
    pub fn has(&self, c: Category, s: Severity) -> bool {
        self.issues.iter().any(|i| i.category == c && i.severity == s)
    }
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    pub fn to_text(&self, max_lines: usize) -> String {
        let mut s = String::new();
        let mut sorted: Vec<&Issue> = self.issues.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        for (n, i) in sorted.iter().enumerate() {
            if n >= max_lines {
                let _ = writeln!(s, "... {} more issue(s) omitted (use --json for full list)", sorted.len() - n);
                break;
            }
            let _ = writeln!(s, "[{}] {} {}: {}", i.severity.as_str(), i.category.as_str(), i.location, i.message);
        }
        for (k, v) in &self.stats {
            let _ = writeln!(s, "stat {k} = {v}");
        }
        let _ = writeln!(
            s,
            "summary: {} error(s), {} warning(s), {} info",
            self.count(Severity::Error),
            self.count(Severity::Warning),
            self.count(Severity::Info)
        );
        s
    }

    pub fn to_json(&self, command: &str) -> String {
        let mut s = String::from("{");
        let _ = write!(s, "\"tool\":\"world-verify\",\"version\":{},", json_str(env!("CARGO_PKG_VERSION")));
        let _ = write!(s, "\"command\":{},", json_str(command));
        let _ = write!(
            s,
            "\"summary\":{{\"errors\":{},\"warnings\":{},\"info\":{}}},",
            self.count(Severity::Error),
            self.count(Severity::Warning),
            self.count(Severity::Info)
        );
        s.push_str("\"stats\":{");
        for (n, (k, v)) in self.stats.iter().enumerate() {
            if n > 0 {
                s.push(',');
            }
            let _ = write!(s, "{}:{}", json_str(k), v);
        }
        s.push_str("},\"issues\":[");
        for (n, i) in self.issues.iter().enumerate() {
            if n > 0 {
                s.push(',');
            }
            let _ = write!(
                s,
                "{{\"severity\":{},\"category\":{},\"location\":{},\"message\":{}}}",
                json_str(i.severity.as_str()),
                json_str(i.category.as_str()),
                json_str(&i.location),
                json_str(&i.message)
            );
        }
        s.push_str("]}\n");
        s
    }
}

pub fn json_str(v: &str) -> String {
    let mut s = String::with_capacity(v.len() + 2);
    s.push('"');
    for c in v.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(s, "\\u{:04x}", c as u32);
            }
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escaping() {
        assert_eq!(json_str("a\"b\\c\n\u{1}"), "\"a\\\"b\\\\c\\n\\u0001\"");
    }

    #[test]
    fn report_counts() {
        let mut r = Report::default();
        r.error(Category::Nbt, "x", "bad");
        r.warn(Category::Nbt, "y", "meh");
        assert_eq!(r.errors(), 1);
        assert!(r.has(Category::Nbt, Severity::Warning));
        assert!(r.to_json("verify").contains("\"errors\":1"));
    }
}
