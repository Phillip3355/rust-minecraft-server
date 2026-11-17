//! Anvil region file (`r.<x>.<z>.mca`) reader and chunk structure validation.
//!
//! Format reference (Minecraft Wiki "Region file format" / "Chunk format"):
//! * 4 KiB location table: 1024 x (3-byte sector offset BE, 1-byte sector count)
//! * 4 KiB timestamp table
//! * chunk payload at offset*4096: u32 BE length (incl. compression byte),
//!   u8 compression type (1 gzip, 2 zlib, 3 none, 4 LZ4, 127 custom;
//!   +128 = payload stored externally in `c.<x>.<z>.mcc`), compressed NBT.

use std::path::{Path, PathBuf};

use crate::compress::{self, DEFAULT_MAX_OUTPUT};
use crate::nbt::{self, Tag};
use crate::report::{Category, Report};

pub const SECTOR: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    /// `region/` — terrain chunks.
    Terrain,
    /// `entities/` — entity chunks (1.17+).
    Entities,
    /// `poi/` — points of interest.
    Poi,
}

#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub cx: i32,
    pub cz: i32,
    pub compression: u8,
    pub nbt: Tag,
}

pub fn parse_region_name(path: &Path) -> Option<(i32, i32)> {
    let name = path.file_name()?.to_str()?;
    let mut it = name.strip_prefix("r.")?.strip_suffix(".mca")?.split('.');
    let x = it.next()?.parse().ok()?;
    let z = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((x, z))
}

pub fn decompress(kind: u8, data: &[u8]) -> Result<Vec<u8>, String> {
    match kind {
        1 => compress::gzip_decompress(data, DEFAULT_MAX_OUTPUT).map_err(|e| e.0),
        2 => compress::zlib_decompress(data, DEFAULT_MAX_OUTPUT).map_err(|e| e.0),
        3 => Ok(data.to_vec()),
        4 => compress::lz4_java_decompress(data, DEFAULT_MAX_OUTPUT).map_err(|e| e.0),
        127 => Err("custom compression (type 127) cannot be verified by this tool".into()),
        k => Err(format!("unknown compression type {k}")),
    }
}

/// Read every chunk of a region file, recording structural problems.
/// Never panics on malformed input.
pub fn read_region(path: &Path, label: &str, report: &mut Report) -> Vec<ChunkRecord> {
    let loc = format!("{label}/{}", path.file_name().map(|s| s.to_string_lossy()).unwrap_or_default());
    let Some((rx, rz)) = parse_region_name(path) else {
        report.warn(Category::RegionHeader, &loc, "file name is not r.<x>.<z>.mca; skipped");
        return Vec::new();
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            report.error(Category::Io, &loc, format!("cannot read: {e}"));
            return Vec::new();
        }
    };
    report.bump("region_files", 1);
    if data.is_empty() {
        report.warn(Category::RegionHeader, &loc, "empty (0-byte) region file");
        return Vec::new();
    }
    if data.len() < 2 * SECTOR {
        report.error(
            Category::RegionHeader,
            &loc,
            format!("truncated header: {} bytes < {} required", data.len(), 2 * SECTOR),
        );
        return Vec::new();
    }
    if data.len() % SECTOR != 0 {
        report.warn(
            Category::RegionSector,
            &loc,
            format!("file size {} is not a multiple of {SECTOR} (unpadded tail)", data.len()),
        );
    }
    let file_sectors = data.len().div_ceil(SECTOR);
    let mut owner: Vec<Option<usize>> = vec![None; file_sectors];
    owner[0] = Some(usize::MAX);
    owner[1] = Some(usize::MAX);

    let mut out = Vec::new();
    for idx in 0..1024usize {
        let e = &data[idx * 4..idx * 4 + 4];
        let offset = ((e[0] as usize) << 16) | ((e[1] as usize) << 8) | e[2] as usize;
        let count = e[3] as usize;
        let (lx, lz) = ((idx % 32) as i32, (idx / 32) as i32);
        let (cx, cz) = (rx * 32 + lx, rz * 32 + lz);
        let cloc = format!("{loc} chunk({cx},{cz})");
        if offset == 0 && count == 0 {
            continue;
        }
        if offset == 0 || count == 0 {
            report.error(Category::RegionHeader, &cloc, format!("half-empty location entry offset={offset} count={count}"));
            continue;
        }
        if offset < 2 {
            report.error(Category::RegionHeader, &cloc, format!("sector offset {offset} overlaps header"));
            continue;
        }
        if offset >= file_sectors {
            report.error(
                Category::RegionSector,
                &cloc,
                format!("sector offset {offset} beyond end of file ({file_sectors} sectors)"),
            );
            continue;
        }
        // Sector ownership (overlap = two chunks share bytes => one is corrupt).
        let mut overlap = None;
        for s in offset..(offset + count).min(file_sectors) {
            if let Some(o) = owner[s] {
                overlap = Some(o);
            } else {
                owner[s] = Some(idx);
            }
        }
        if let Some(o) = overlap {
            let other = if o == usize::MAX {
                "header".to_string()
            } else {
                format!("chunk({},{})", rx * 32 + (o % 32) as i32, rz * 32 + (o / 32) as i32)
            };
            report.error(Category::RegionSector, &cloc, format!("sectors overlap with {other}"));
        }
        let start = offset * SECTOR;
        if start + 5 > data.len() {
            report.error(Category::RegionSector, &cloc, "chunk header truncated by end of file");
            continue;
        }
        let length = u32::from_be_bytes(data[start..start + 4].try_into().unwrap()) as usize;
        let ctype = data[start + 4];
        let external = ctype & 0x80 != 0;
        let ctype_base = ctype & 0x7F;
        if length == 0 {
            report.error(Category::RegionSector, &cloc, "chunk length field is 0");
            continue;
        }
        let payload: Vec<u8> = if external {
            if length != 1 {
                report.warn(Category::RegionSector, &cloc, format!("external chunk has inline length {length} (expected 1)"));
            }
            let mcc: PathBuf = path.with_file_name(format!("c.{cx}.{cz}.mcc"));
            match std::fs::read(&mcc) {
                Ok(d) => {
                    report.bump("external_chunks", 1);
                    d
                }
                Err(e) => {
                    report.error(Category::Io, &cloc, format!("external chunk file {} unreadable: {e}", mcc.display()));
                    continue;
                }
            }
        } else {
            if 4 + length > count * SECTOR {
                report.error(
                    Category::RegionSector,
                    &cloc,
                    format!("length {length} exceeds allocated {count} sector(s)"),
                );
                continue;
            }
            if start + 4 + length > data.len() {
                report.error(Category::RegionSector, &cloc, format!("payload truncated: needs {} bytes past EOF", start + 4 + length - data.len()));
                continue;
            }
            data[start + 5..start + 4 + length].to_vec()
        };
        let raw = match decompress(ctype_base, &payload) {
            Ok(r) => r,
            Err(m) => {
                let cat = if ctype_base == 127 { Category::Unsupported } else { Category::Compression };
                report.error(cat, &cloc, m);
                continue;
            }
        };
        match nbt::read(&raw) {
            Ok((_, tag)) => {
                report.bump(&format!("compression_type_{ctype_base}"), 1);
                out.push(ChunkRecord { cx, cz, compression: ctype_base, nbt: tag });
            }
            Err(e) => report.error(Category::Nbt, &cloc, e.0),
        }
    }
    out
}

fn ceil_log2(n: usize) -> u32 {
    if n <= 1 { 0 } else { usize::BITS - (n - 1).leading_zeros() }
}

/// Unpack a 1.16+ (non-spanning) packed long array.
pub fn unpack(data: &[i64], bits: u32, n: usize) -> Vec<u32> {
    let per = (64 / bits) as usize;
    let mask = (1u64 << bits) - 1;
    (0..n)
        .map(|i| {
            let word = data[i / per] as u64;
            ((word >> ((i % per) as u32 * bits)) & mask) as u32
        })
        .collect()
}

/// Shared palette-container validation for block_states / biomes.
/// Returns palette indices on success.
fn check_container(
    c: &Tag,
    entries: usize,
    min_bits: u32,
    what: &str,
    loc: &str,
    report: &mut Report,
    cat: Category,
) -> Option<(Vec<Tag>, Vec<u32>)> {
    let Some(palette) = c.get("palette").and_then(Tag::as_list) else {
        report.error(cat, loc, format!("{what}: missing palette list"));
        return None;
    };
    if palette.is_empty() {
        report.error(cat, loc, format!("{what}: empty palette"));
        return None;
    }
    if palette.len() == 1 {
        return Some((palette.to_vec(), vec![0; entries]));
    }
    let Some(data) = c.get("data").and_then(Tag::as_long_array) else {
        report.error(cat, loc, format!("{what}: palette has {} entries but data array missing", palette.len()));
        return None;
    };
    let bits = ceil_log2(palette.len()).max(min_bits);
    let per = (64 / bits) as usize;
    let expected = entries.div_ceil(per);
    if data.len() != expected {
        report.error(
            cat,
            loc,
            format!("{what}: data has {} longs, expected {expected} for {} palette entries ({bits} bits)", data.len(), palette.len()),
        );
        return None;
    }
    let idx = unpack(data, bits, entries);
    if let Some((pos, &bad)) = idx.iter().enumerate().find(|(_, &v)| v as usize >= palette.len()) {
        report.error(
            cat,
            loc,
            format!("{what}: index {bad} at entry {pos} out of palette range {}", palette.len()),
        );
        return None;
    }
    Some((palette.to_vec(), idx))
}

/// Decoded block states for one section: (palette, 4096 indices in YZX order).
pub fn section_blocks(section: &Tag) -> Option<(Vec<Tag>, Vec<u32>)> {
    let mut sink = Report::default();
    check_container(section.get("block_states")?, 4096, 4, "block_states", "", &mut sink, Category::BlockStates)
}

/// Validate a terrain chunk's NBT structure. `cx/cz` come from the region slot.
pub fn validate_terrain_chunk(ch: &ChunkRecord, loc: &str, report: &mut Report) {
    let t = &ch.nbt;
    report.bump("chunks", 1);
    match t.get("DataVersion").and_then(Tag::as_i64) {
        Some(v) => report.bump(&format!("data_version_{v}"), 1),
        None => report.error(Category::ChunkStructure, loc, "missing DataVersion"),
    }
    let x = t.get("xPos").and_then(Tag::as_i64);
    let z = t.get("zPos").and_then(Tag::as_i64);
    match (x, z) {
        (Some(x), Some(z)) if x == ch.cx as i64 && z == ch.cz as i64 => {}
        (Some(x), Some(z)) => report.error(
            Category::ChunkStructure,
            loc,
            format!("xPos/zPos ({x},{z}) do not match region slot ({},{}) — misplaced chunk", ch.cx, ch.cz),
        ),
        _ => report.error(Category::ChunkStructure, loc, "missing xPos/zPos"),
    }
    match t.get("Status").and_then(Tag::as_str) {
        Some(s) if s == "minecraft:full" || s == "full" => report.bump("chunks_full", 1),
        Some(_) => report.bump("chunks_proto", 1),
        None => report.error(Category::ChunkStructure, loc, "missing Status"),
    }
    let Some(sections) = t.get("sections").and_then(Tag::as_list) else {
        report.error(Category::ChunkStructure, loc, "missing sections list");
        return;
    };
    let mut ys = Vec::new();
    for s in sections {
        let Some(y) = s.get("Y").and_then(Tag::as_i64) else {
            report.error(Category::ChunkStructure, loc, "section without Y");
            continue;
        };
        if ys.contains(&y) {
            report.error(Category::ChunkStructure, loc, format!("duplicate section Y={y}"));
        }
        ys.push(y);
        let sloc = format!("{loc} section Y={y}");
        if let Some(bs) = s.get("block_states") {
            if let Some((pal, _)) = check_container(bs, 4096, 4, "block_states", &sloc, report, Category::BlockStates) {
                for p in &pal {
                    if p.get("Name").and_then(Tag::as_str).is_none_or(str::is_empty) {
                        report.error(Category::BlockStates, &sloc, "palette entry without Name");
                    }
                }
            }
        }
        if let Some(b) = s.get("biomes") {
            if let Some((pal, _)) = check_container(b, 64, 1, "biomes", &sloc, report, Category::Biomes) {
                if pal.iter().any(|p| p.as_str().is_none()) {
                    report.error(Category::Biomes, &sloc, "biome palette entry is not a string");
                }
            }
        }
        report.bump("sections", 1);
    }
    if let Some(bes) = t.get("block_entities").and_then(Tag::as_list) {
        for be in bes {
            report.bump("block_entities", 1);
            let id = be.get("id").and_then(Tag::as_str);
            let (bx, by, bz) = (
                be.get("x").and_then(Tag::as_i64),
                be.get("y").and_then(Tag::as_i64),
                be.get("z").and_then(Tag::as_i64),
            );
            match (id, bx, by, bz) {
                (Some(_), Some(bx), Some(_), Some(bz)) => {
                    if bx.div_euclid(16) != ch.cx as i64 || bz.div_euclid(16) != ch.cz as i64 {
                        report.error(
                            Category::BlockEntity,
                            loc,
                            format!("block entity {} at x={bx} z={bz} lies outside this chunk", id.unwrap()),
                        );
                    }
                }
                _ => report.error(Category::BlockEntity, loc, "block entity missing id or x/y/z"),
            }
        }
    }
}

/// Validate an entity chunk (entities/ folder).
pub fn validate_entity_chunk(ch: &ChunkRecord, loc: &str, report: &mut Report) {
    report.bump("entity_chunks", 1);
    if ch.nbt.get("DataVersion").and_then(Tag::as_i64).is_none() {
        report.error(Category::ChunkStructure, loc, "entity chunk missing DataVersion");
    }
    match ch.nbt.get("Position") {
        Some(Tag::IntArray(p)) if p.len() == 2 => {
            if p[0] != ch.cx || p[1] != ch.cz {
                report.error(
                    Category::ChunkStructure,
                    loc,
                    format!("entity chunk Position [{},{}] does not match slot ({},{})", p[0], p[1], ch.cx, ch.cz),
                );
            }
        }
        _ => report.error(Category::ChunkStructure, loc, "entity chunk missing Position int[2]"),
    }
    if let Some(ents) = ch.nbt.get("Entities").and_then(Tag::as_list) {
        report.bump("entities", ents.len() as i64);
    }
}

// ------------------------------------------------------- fixture writer

/// Build a region file from chunk NBTs (fixtures/tests only). Each chunk is
/// compressed with `ctype` using the stored/literal-only encoders.
pub fn write_region(chunks: &[(i32, i32, Tag)], ctype: u8) -> Vec<u8> {
    let mut out = vec![0u8; 2 * SECTOR];
    let mut next = 2usize;
    for (cx, cz, tag) in chunks {
        let raw = nbt::write("", tag);
        let body = match ctype {
            1 => compress::gzip_compress_stored(&raw),
            2 => compress::zlib_compress_stored(&raw),
            3 => raw,
            4 => compress::lz4_java_compress_literals(&raw),
            _ => panic!("unsupported fixture compression {ctype}"),
        };
        let len = body.len() + 1;
        let sectors = (len + 4).div_ceil(SECTOR);
        assert!(sectors < 256, "fixture chunk too large");
        let idx = (cx.rem_euclid(32) + cz.rem_euclid(32) * 32) as usize;
        out[idx * 4] = (next >> 16) as u8;
        out[idx * 4 + 1] = (next >> 8) as u8;
        out[idx * 4 + 2] = next as u8;
        out[idx * 4 + 3] = sectors as u8;
        out.extend_from_slice(&(len as u32).to_be_bytes());
        out.push(ctype);
        out.extend_from_slice(&body);
        out.resize((next + sectors) * SECTOR, 0);
        next += sectors;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_name_parsing() {
        assert_eq!(parse_region_name(Path::new("r.-1.2.mca")), Some((-1, 2)));
        assert_eq!(parse_region_name(Path::new("r.1.mca")), None);
        assert_eq!(parse_region_name(Path::new("r.1.2.mcr")), None);
    }

    #[test]
    fn ceil_log2_values() {
        assert_eq!(ceil_log2(1), 0);
        assert_eq!(ceil_log2(2), 1);
        assert_eq!(ceil_log2(16), 4);
        assert_eq!(ceil_log2(17), 5);
    }

    #[test]
    fn unpack_non_spanning() {
        // 5 bits -> 12 entries per long, top 4 bits unused
        let mut word: u64 = 0;
        for i in 0..12u64 {
            word |= (i + 1) << (i * 5);
        }
        let v = unpack(&[word as i64], 5, 12);
        assert_eq!(v, (1..=12).collect::<Vec<u32>>());
    }
}
