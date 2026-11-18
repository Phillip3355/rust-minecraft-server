//! World directory scanning, player data validation, and snapshot diffing.
//!
//! The scanner is layout-agnostic: it walks the world directory looking for
//! folders named `region`, `entities`, `poi` and `playerdata`, so it works for
//! the classic layout (`region/`, `DIM-1/region/`, `DIM1/region/`), for
//! `dimensions/<ns>/<name>/region/`, and for whatever layout Pumpkin uses at
//! the pinned commit (to be confirmed — see docs/PHASE1.md).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::anvil::{self, ChunkRecord};
use crate::compress::{self, DEFAULT_MAX_OUTPUT};
use crate::items::{self, Totals};
use crate::nbt::{self, Tag};
use crate::report::{Category, Report};

/// Keys that legitimately change on every save and are ignored by `diff`.
pub const VOLATILE_CHUNK_KEYS: &[&str] = &["LastUpdate", "InhabitedTime"];
pub const MAX_WALK_DEPTH: usize = 8;

#[derive(Debug, Default)]
pub struct World {
    pub root: PathBuf,
    /// "<dimension label>" -> (cx,cz) -> terrain chunk
    pub terrain: BTreeMap<String, BTreeMap<(i32, i32), ChunkRecord>>,
    pub entities: BTreeMap<String, BTreeMap<(i32, i32), ChunkRecord>>,
    /// uuid (file stem) -> player NBT
    pub players: BTreeMap<String, Tag>,
    pub level: Option<Tag>,
    pub report: Report,
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth > MAX_WALK_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        // Do not follow symlinks (avoid cycles / escaping the world dir).
        let Ok(md) = std::fs::symlink_metadata(&p) else { continue };
        if md.is_dir() {
            found.push(p.clone());
            walk(&p, depth + 1, found);
        }
    }
}

fn dim_label(root: &Path, folder: &Path) -> String {
    let parent = folder.parent().unwrap_or(root);
    match parent.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => "overworld".to_string(),
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => parent.to_string_lossy().into_owned(),
    }
}

fn files_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == ext)).collect())
        .unwrap_or_default();
    v.sort();
    v
}

pub fn read_gzip_nbt(path: &Path) -> Result<Tag, String> {
    let raw = std::fs::read(path).map_err(|e| format!("cannot read: {e}"))?;
    // Vanilla writes gzip; accept zlib/uncompressed defensively and say so.
    let data = if raw.starts_with(&[0x1F, 0x8B]) {
        compress::gzip_decompress(&raw, DEFAULT_MAX_OUTPUT).map_err(|e| e.0)?
    } else if raw.first() == Some(&0x78) {
        compress::zlib_decompress(&raw, DEFAULT_MAX_OUTPUT).map_err(|e| e.0)?
    } else {
        raw
    };
    nbt::read(&data).map(|(_, t)| t).map_err(|e| e.0)
}

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, &c)| match i {
            8 | 13 | 18 | 23 => c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn check_slots(list: &[Tag], key: &str, allowed: &dyn Fn(i64) -> bool, loc: &str, report: &mut Report) {
    let mut seen: Vec<i64> = Vec::new();
    for it in list {
        match it.get("Slot").and_then(Tag::as_i64) {
            Some(s) => {
                if seen.contains(&s) {
                    report.error(Category::PlayerData, loc, format!("{key}: duplicate Slot {s} (duplication/corruption)"));
                }
                if !allowed(s) {
                    report.warn(Category::PlayerData, loc, format!("{key}: unexpected Slot {s}"));
                }
                seen.push(s);
            }
            None => report.error(Category::PlayerData, loc, format!("{key}: item without Slot")),
        }
    }
}

pub fn validate_player(uuid: &str, t: &Tag, report: &mut Report) {
    let loc = format!("playerdata/{uuid}.dat");
    report.bump("players", 1);
    if !is_uuid(uuid) {
        report.warn(Category::PlayerData, &loc, "file name is not a hyphenated UUID");
    }
    if t.get("DataVersion").and_then(Tag::as_i64).is_none() {
        report.warn(Category::PlayerData, &loc, "missing DataVersion");
    }
    match t.get("Pos").and_then(Tag::as_list) {
        Some(p) if p.len() == 3 => {
            let v: Vec<Option<f64>> = p.iter().map(Tag::as_f64).collect();
            if v.iter().any(|x| x.is_none_or(|f| !f.is_finite())) {
                report.error(Category::PlayerData, &loc, "Pos contains non-finite value");
            } else if v[0].unwrap().abs() > 30_000_000.0 || v[2].unwrap().abs() > 30_000_000.0 {
                report.error(Category::PlayerData, &loc, "Pos outside world border limit (±30,000,000)");
            } else if !(-2048.0..=2048.0).contains(&v[1].unwrap()) {
                report.warn(Category::PlayerData, &loc, format!("Pos Y={} far outside build range", v[1].unwrap()));
            }
        }
        _ => report.error(Category::PlayerData, &loc, "missing or malformed Pos (expected list of 3)"),
    }
    match t.get("Health").and_then(Tag::as_f64) {
        Some(h) if h.is_finite() && h >= 0.0 => {}
        Some(h) => report.error(Category::PlayerData, &loc, format!("invalid Health {h}")),
        None => report.warn(Category::PlayerData, &loc, "missing Health"),
    }
    if let Some(f) = t.get("foodLevel").and_then(Tag::as_i64) {
        if !(0..=20).contains(&f) {
            report.error(Category::PlayerData, &loc, format!("foodLevel {f} outside 0..=20"));
        }
    }
    if let Some(inv) = t.get("Inventory").and_then(Tag::as_list) {
        // 0..=35 main, 100..=103 armor and -106 offhand (pre-1.21.5 layout).
        check_slots(inv, "Inventory", &|s| (0..=35).contains(&s) || (100..=103).contains(&s) || s == -106, &loc, report);
    } else if t.get("Inventory").is_some() {
        report.error(Category::PlayerData, &loc, "Inventory is not a list");
    } else {
        report.warn(Category::PlayerData, &loc, "missing Inventory");
    }
    if let Some(ender) = t.get("EnderItems").and_then(Tag::as_list) {
        check_slots(ender, "EnderItems", &|s| (0..=26).contains(&s), &loc, report);
    }
}

pub fn load(root: &Path) -> World {
    let mut w = World { root: root.to_path_buf(), ..Default::default() };
    if !root.is_dir() {
        w.report.error(Category::Io, root.display().to_string(), "world directory does not exist");
        return w;
    }
    let level = root.join("level.dat");
    if level.is_file() {
        match read_gzip_nbt(&level) {
            Ok(t) => {
                if t.get("Data").and_then(Tag::as_compound).is_none() {
                    w.report.error(Category::LevelDat, "level.dat", "root has no Data compound");
                }
                w.level = Some(t);
            }
            Err(e) => w.report.error(Category::LevelDat, "level.dat", e),
        }
    } else {
        w.report.error(Category::LevelDat, "level.dat", "missing level.dat");
    }

    let mut dirs = Vec::new();
    walk(root, 0, &mut dirs);
    for d in dirs {
        let name = d.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let label = dim_label(root, &d);
        match name {
            "region" | "entities" => {
                let is_terrain = name == "region";
                for f in files_with_ext(&d, "mca") {
                    let rloc = format!("{label}/{name}");
                    for ch in anvil::read_region(&f, &rloc, &mut w.report) {
                        let cloc = format!("{rloc} chunk({},{})", ch.cx, ch.cz);
                        if is_terrain {
                            anvil::validate_terrain_chunk(&ch, &cloc, &mut w.report);
                            w.terrain.entry(label.clone()).or_default().insert((ch.cx, ch.cz), ch);
                        } else {
                            anvil::validate_entity_chunk(&ch, &cloc, &mut w.report);
                            w.entities.entry(label.clone()).or_default().insert((ch.cx, ch.cz), ch);
                        }
                    }
                }
            }
            "poi" => {
                for f in files_with_ext(&d, "mca") {
                    let n = anvil::read_region(&f, &format!("{label}/poi"), &mut w.report).len();
                    w.report.bump("poi_chunks", n as i64);
                }
            }
            "playerdata" => {
                for f in files_with_ext(&d, "dat") {
                    let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    match read_gzip_nbt(&f) {
                        Ok(t) => {
                            validate_player(&stem, &t, &mut w.report);
                            if w.players.insert(stem.clone(), t).is_some() {
                                w.report.error(Category::PlayerData, &stem, "same player UUID found in two playerdata folders");
                            }
                        }
                        Err(e) => w.report.error(Category::PlayerData, format!("playerdata/{stem}.dat"), e),
                    }
                }
                let tmp = files_with_ext(&d, "tmp");
                if !tmp.is_empty() {
                    w.report.warn(
                        Category::PlayerData,
                        "playerdata",
                        format!("{} leftover .tmp file(s) — interrupted save?", tmp.len()),
                    );
                }
            }
            _ => {}
        }
    }
    w
}

impl World {
    pub fn item_totals(&mut self) -> Totals {
        let mut total = Totals::default();
        let mut sink = Report::default();
        for (uuid, p) in &self.players {
            let mut t = Totals::default();
            items::collect(p, &format!("player {uuid}"), &mut t, &mut sink);
            total.merge(&t);
        }
        for (dim, chunks) in self.terrain.iter().chain(self.entities.iter()) {
            for ((cx, cz), ch) in chunks {
                let loc = format!("{dim} chunk({cx},{cz})");
                if let Some(be) = ch.nbt.get("block_entities") {
                    items::collect(be, &loc, &mut total, &mut sink);
                }
                if let Some(e) = ch.nbt.get("Entities") {
                    items::collect(e, &loc, &mut total, &mut sink);
                }
            }
        }
        self.report.merge(sink);
        total
    }
}

fn first_block_diff(a: &Tag, b: &Tag) -> Option<String> {
    let sec = |t: &Tag| -> BTreeMap<i64, Tag> {
        t.get("sections")
            .and_then(Tag::as_list)
            .unwrap_or(&[])
            .iter()
            .filter_map(|s| Some((s.get("Y")?.as_i64()?, s.clone())))
            .collect()
    };
    let (sa, sb) = (sec(a), sec(b));
    let mut ys: Vec<i64> = sa.keys().chain(sb.keys()).copied().collect();
    ys.sort();
    ys.dedup();
    for y in ys {
        let da = sa.get(&y).and_then(anvil::section_blocks);
        let db = sb.get(&y).and_then(anvil::section_blocks);
        let is_all_air = |d: &Option<(Vec<Tag>, Vec<u32>)>| match d {
            None => true,
            Some((p, _)) => {
                p.len() == 1 && p[0].get("Name").and_then(Tag::as_str).is_some_and(|n| n.ends_with("air"))
            }
        };
        match (&da, &db) {
            (Some((pa, ia)), Some((pb, ib))) => {
                for i in 0..4096 {
                    let (x, z, yy) = (i % 16, (i / 16) % 16, i / 256);
                    let ba = &pa[ia[i] as usize];
                    let bb = &pb[ib[i] as usize];
                    if ba != bb && !ba.eq_ignoring(bb, &[]) {
                        let name = |t: &Tag| t.get("Name").and_then(Tag::as_str).unwrap_or("?").to_string();
                        return Some(format!(
                            "section Y={y} local ({x},{yy},{z}): {} -> {}",
                            name(ba),
                            name(bb)
                        ));
                    }
                }
            }
            _ if is_all_air(&da) && is_all_air(&db) => {}
            _ => return Some(format!("section Y={y} present/decodable in only one snapshot")),
        }
    }
    None
}

/// Compare two snapshots of the same world (e.g. before/after restart).
/// `strict` = no gameplay happened in between, so blocks and players must
/// be identical (modulo volatile keys).
pub fn diff(a: &World, b: &World, strict: bool, report: &mut Report) {
    let seed = |w: &World| -> Option<i64> {
        let d = w.level.as_ref()?.get("Data")?;
        d.get("WorldGenSettings").and_then(|g| g.get("seed")).or_else(|| d.get("RandomSeed")).and_then(Tag::as_i64)
    };
    if let (Some(x), Some(y)) = (seed(a), seed(b)) {
        if x != y {
            report.error(Category::LevelDat, "level.dat", format!("world seed changed {x} -> {y}"));
        }
    }
    for (dim, chunks) in &a.terrain {
        let other = b.terrain.get(dim);
        for (pos, ca) in chunks {
            let loc = format!("{dim} chunk({},{})", pos.0, pos.1);
            report.bump("diff_chunks_compared", 1);
            let Some(cb) = other.and_then(|m| m.get(pos)) else {
                report.error(Category::ChunkLost, &loc, "chunk present before, missing after (data loss)");
                continue;
            };
            if let Some(d) = first_block_diff(&ca.nbt, &cb.nbt) {
                if strict {
                    report.error(Category::BlockChanged, &loc, d);
                } else {
                    report.bump("diff_chunks_changed", 1);
                }
            }
            let bes = |c: &ChunkRecord| c.nbt.get("block_entities").cloned().unwrap_or(Tag::List(0, vec![]));
            if strict && !bes(ca).eq_ignoring(&bes(cb), VOLATILE_CHUNK_KEYS) {
                report.error(Category::BlockChanged, &loc, "block entities differ");
            }
        }
    }
    for (dim, chunks) in &b.terrain {
        let n = chunks.keys().filter(|k| a.terrain.get(dim).is_none_or(|m| !m.contains_key(k))).count();
        if n > 0 {
            report.info(Category::ChunkStructure, dim.as_str(), format!("{n} chunk(s) new in second snapshot"));
        }
    }
    for (uuid, pa) in &a.players {
        let loc = format!("player {uuid}");
        let Some(pb) = b.players.get(uuid) else {
            report.error(Category::PlayerLost, &loc, "player data present before, missing after");
            continue;
        };
        if !strict {
            continue;
        }
        for key in ["Inventory", "EnderItems", "equipment", "XpLevel", "XpTotal", "foodLevel", "Dimension"] {
            match (pa.get(key), pb.get(key)) {
                (None, None) => {}
                (Some(x), Some(y)) if x.eq_ignoring(y, &[]) => {}
                _ => report.error(Category::PlayerChanged, &loc, format!("{key} differs across save/reload")),
            }
        }
        let pos = |p: &Tag| -> Option<Vec<f64>> { p.get("Pos")?.as_list()?.iter().map(Tag::as_f64).collect() };
        if let (Some(x), Some(y)) = (pos(pa), pos(pb)) {
            let d = x.iter().zip(&y).map(|(p, q)| (p - q).powi(2)).sum::<f64>().sqrt();
            if d > 0.01 {
                report.error(Category::PlayerChanged, &loc, format!("Pos moved {d:.3} blocks across save/reload"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_check() {
        assert!(is_uuid("069a79f4-44e9-4726-a5be-fca90e38aaf5"));
        assert!(!is_uuid("069a79f444e94726a5befca90e38aaf5"));
    }
}
