//! Phase 1 regression suite for the verifier itself.
//!
//! Each test writes a synthetic world (see `fixture.rs`), injects one specific
//! failure class from the Phase 1 plan (crash-truncated region, chunk
//! corruption, item duplication, player data loss, ...) and asserts that the
//! verifier reports it under the expected category. A clean world and a
//! byte-identical "save/reload" copy must report zero errors.
//!
//! These tests prove the *detector* works. They do NOT prove Pumpkin writes
//! correct worlds — that requires running `scripts/phase1/save_reload_check.sh`
//! against snapshots produced by the pinned Pumpkin binary (see docs/PHASE1.md).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use world_verify::anvil;
use world_verify::compress;
use world_verify::fixture::{self, PLAYER_UUID};
use world_verify::items;
use world_verify::nbt::{self, Tag};
use world_verify::report::{Category, Report, Severity};
use world_verify::world;

static SEQ: AtomicU32 = AtomicU32::new(0);

/// Unique temp dir, removed on drop.
struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("world-verify-test-{}-{tag}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture_world(tag: &str, ctype: u8) -> TempDir {
    let d = TempDir::new(tag);
    fixture::write_world(d.path(), ctype).unwrap();
    d
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap().flatten() {
        let p = e.path();
        let t = dst.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            std::fs::copy(&p, &t).unwrap();
        }
    }
}

fn errors_in(r: &Report, c: Category) -> usize {
    r.issues.iter().filter(|i| i.category == c && i.severity == Severity::Error).count()
}

fn dump(r: &Report) -> String {
    r.to_text(100)
}

fn write_player(dir: &Path, inv: Vec<Tag>) {
    let p = fixture::player(inv);
    std::fs::write(
        dir.join(format!("playerdata/{PLAYER_UUID}.dat")),
        compress::gzip_compress_stored(&nbt::write("", &p)),
    )
    .unwrap();
}

fn default_inventory() -> Vec<Tag> {
    vec![
        fixture::slot_item(0, "minecraft:iron_pickaxe", 1),
        fixture::slot_item(1, "minecraft:bread", 12),
        fixture::slot_item(100, "minecraft:iron_boots", 1),
    ]
}

// ------------------------------------------------------------ clean baseline

#[test]
fn clean_world_all_compressions_have_no_errors() {
    for ctype in 1..=4u8 {
        let d = fixture_world("clean", ctype);
        let w = world::load(d.path());
        assert_eq!(w.report.errors(), 0, "ctype {ctype}:\n{}", dump(&w.report));
        assert_eq!(w.terrain.get("overworld").map(|m| m.len()), Some(2), "ctype {ctype}");
        assert_eq!(w.terrain.get("DIM-1").map(|m| m.len()), Some(1), "nether chunk, ctype {ctype}");
        assert_eq!(w.players.len(), 1);
        assert_eq!(w.report.stats.get(&format!("compression_type_{ctype}")).copied(), Some(4), "ctype {ctype}");
    }
}

#[test]
fn identical_save_reload_strict_diff_is_clean() {
    let a = fixture_world("reload-a", 2);
    let b = TempDir::new("reload-b");
    copy_dir(a.path(), b.path());
    let (wa, wb) = (world::load(a.path()), world::load(b.path()));
    let mut r = Report::default();
    world::diff(&wa, &wb, true, &mut r);
    assert_eq!(r.errors(), 0, "{}", dump(&r));
    assert_eq!(r.stats.get("diff_chunks_compared").copied(), Some(3));
}

#[test]
fn recompression_is_not_a_change() {
    // Same logical content written with zlib vs LZ4 (e.g. after a config change)
    // must diff clean: diff compares decoded NBT, not bytes.
    let a = fixture_world("recomp-a", 2);
    let b = fixture_world("recomp-b", 4);
    let mut r = Report::default();
    world::diff(&world::load(a.path()), &world::load(b.path()), true, &mut r);
    assert_eq!(r.errors(), 0, "{}", dump(&r));
}

#[test]
fn item_totals_match_fixture_contents() {
    let d = fixture_world("totals", 2);
    let mut w = world::load(d.path());
    let t = w.item_totals();
    let get = |id: &str| t.by_id.get(id).copied().unwrap_or(0);
    assert_eq!(get("minecraft:diamond"), 5); // chest
    assert_eq!(get("minecraft:cobblestone"), 64); // chest
    assert_eq!(get("minecraft:oak_log"), 7); // item entity
    assert_eq!(get("minecraft:bread"), 12); // player inventory
    assert_eq!(get("minecraft:ender_pearl"), 4); // ender chest
    assert_eq!(get("minecraft:iron_boots"), 1); // armor slot
    assert_eq!(w.report.errors(), 0, "{}", dump(&w.report));
}

// ----------------------------------------------- crash / chunk corruption

#[test]
fn crash_truncated_region_detected() {
    let d = fixture_world("trunc", 2);
    let f = d.path().join("region/r.0.0.mca");
    let data = std::fs::read(&f).unwrap();
    // Simulate a crash mid-write: keep header + part of the first chunk sector.
    std::fs::write(&f, &data[..2 * anvil::SECTOR + 100]).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::RegionSector) > 0, "{}", dump(&w.report));
}

#[test]
fn crash_truncated_header_detected() {
    let d = fixture_world("trunc-hdr", 2);
    let f = d.path().join("region/r.0.0.mca");
    let data = std::fs::read(&f).unwrap();
    std::fs::write(&f, &data[..1000]).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::RegionHeader) > 0, "{}", dump(&w.report));
}

fn first_chunk_payload_offset(region: &[u8]) -> usize {
    let off = ((region[0] as usize) << 16) | ((region[1] as usize) << 8) | region[2] as usize;
    off * anvil::SECTOR + 5
}

#[test]
fn bitflip_in_compressed_chunk_detected_for_checksummed_formats() {
    // gzip (CRC32), zlib (Adler-32), LZ4 (XXH32) all carry checksums; a flip in
    // the payload must be caught at the compression or NBT layer.
    for ctype in [1u8, 2, 4] {
        let d = fixture_world("flip", ctype);
        let f = d.path().join("region/r.0.0.mca");
        let mut data = std::fs::read(&f).unwrap();
        let p = first_chunk_payload_offset(&data) + 200;
        data[p] ^= 0x10;
        std::fs::write(&f, &data).unwrap();
        let w = world::load(d.path());
        let n = errors_in(&w.report, Category::Compression) + errors_in(&w.report, Category::Nbt);
        assert!(n > 0, "ctype {ctype} not detected:\n{}", dump(&w.report));
    }
}

#[test]
fn overlapping_sectors_detected() {
    let d = fixture_world("overlap", 2);
    let f = d.path().join("region/r.0.0.mca");
    let mut data = std::fs::read(&f).unwrap();
    // Point chunk index 1 (chunk 1,0) at chunk 0's sectors.
    let e0: [u8; 4] = data[0..4].try_into().unwrap();
    data[4..8].copy_from_slice(&e0);
    std::fs::write(&f, &data).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::RegionSector) > 0, "{}", dump(&w.report));
    // The aliased record decodes as chunk (0,0) at slot (1,0): coordinate mismatch.
    assert!(errors_in(&w.report, Category::ChunkStructure) > 0, "{}", dump(&w.report));
}

#[test]
fn chunk_coordinate_mismatch_detected() {
    let d = fixture_world("coord", 2);
    // Store chunk claiming xPos=5 in slot (0,0).
    let bad = fixture::chunk(5, 0, vec![]);
    std::fs::write(d.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, bad)], 2)).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::ChunkStructure) > 0, "{}", dump(&w.report));
}

#[test]
fn palette_index_out_of_range_detected() {
    // Note: a 1-entry palette legitimately ignores `data` (vanilla), so the
    // corruption injected here is a packed index (3) beyond a 2-entry palette.
    let d = fixture_world("palette", 2);
    let mut idx: Vec<u32> = (0..4096).map(|i| if i / 256 < 8 { 1 } else { 0 }).collect();
    idx[1234] = 3;
    let bad_section = Tag::Compound(vec![
        ("Y".into(), Tag::Byte(0)),
        (
            "block_states".into(),
            Tag::Compound(vec![
                (
                    "palette".into(),
                    Tag::List(10, vec![
                        Tag::Compound(vec![("Name".into(), Tag::String("minecraft:air".into()))]),
                        Tag::Compound(vec![("Name".into(), Tag::String("minecraft:stone".into()))]),
                    ]),
                ),
                ("data".into(), Tag::LongArray(fixture::pack(&idx, 4))),
            ]),
        ),
        ("biomes".into(), Tag::Compound(vec![("palette".into(), Tag::List(8, vec![Tag::String("minecraft:plains".into())]))])),
    ]);
    let mut c = fixture::chunk(0, 0, vec![]);
    if let Tag::Compound(kv) = &mut c {
        for (k, v) in kv.iter_mut() {
            if k == "sections" {
                *v = Tag::List(10, vec![bad_section.clone()]);
            }
        }
    }
    std::fs::write(d.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, c)], 2)).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::BlockStates) > 0, "{}", dump(&w.report));
}

#[test]
fn missing_level_dat_detected() {
    let d = fixture_world("nolevel", 2);
    std::fs::remove_file(d.path().join("level.dat")).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::LevelDat) > 0);
}

#[test]
fn corrupt_level_dat_detected() {
    let d = fixture_world("badlevel", 2);
    let f = d.path().join("level.dat");
    let mut data = std::fs::read(&f).unwrap();
    let n = data.len();
    data[n - 6] ^= 0xFF; // inside gzip CRC32 trailer
    std::fs::write(&f, &data).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::LevelDat) > 0, "{}", dump(&w.report));
}

#[test]
fn verifier_never_panics_on_garbage_region() {
    let d = fixture_world("garbage", 2);
    let mut x: u32 = 0x1234_5678;
    let junk: Vec<u8> = (0..3 * anvil::SECTOR)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect();
    std::fs::write(d.path().join("region/r.0.0.mca"), &junk).unwrap();
    let w = world::load(d.path());
    assert!(w.report.errors() > 0);
}

// ------------------------------------------------- player data integrity

#[test]
fn player_data_loss_detected_by_diff() {
    let a = fixture_world("ploss-a", 2);
    let b = TempDir::new("ploss-b");
    copy_dir(a.path(), b.path());
    std::fs::remove_file(b.path().join(format!("playerdata/{PLAYER_UUID}.dat"))).unwrap();
    let mut r = Report::default();
    world::diff(&world::load(a.path()), &world::load(b.path()), false, &mut r);
    assert!(errors_in(&r, Category::PlayerLost) > 0, "{}", dump(&r));
}

#[test]
fn player_inventory_change_across_reload_detected_in_strict_mode() {
    let a = fixture_world("pinv-a", 2);
    let b = TempDir::new("pinv-b");
    copy_dir(a.path(), b.path());
    let mut inv = default_inventory();
    inv.pop(); // boots vanished across save/reload
    write_player(b.path(), inv);
    let (wa, wb) = (world::load(a.path()), world::load(b.path()));
    let mut strict = Report::default();
    world::diff(&wa, &wb, true, &mut strict);
    assert!(errors_in(&strict, Category::PlayerChanged) > 0, "{}", dump(&strict));
    // Non-strict (gameplay happened) must not flag inventory edits as errors.
    let mut loose = Report::default();
    world::diff(&wa, &wb, false, &mut loose);
    assert_eq!(errors_in(&loose, Category::PlayerChanged), 0);
}

#[test]
fn duplicate_inventory_slot_detected() {
    let d = fixture_world("dupslot", 2);
    let mut inv = default_inventory();
    inv.push(fixture::slot_item(1, "minecraft:diamond", 3)); // slot 1 used twice
    write_player(d.path(), inv);
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::PlayerData) > 0, "{}", dump(&w.report));
}

#[test]
fn non_finite_position_detected() {
    let d = fixture_world("nan", 2);
    let mut p = fixture::player(default_inventory());
    if let Tag::Compound(kv) = &mut p {
        for (k, v) in kv.iter_mut() {
            if k == "Pos" {
                *v = Tag::List(6, vec![Tag::Double(f64::NAN), Tag::Double(64.0), Tag::Double(0.0)]);
            }
        }
    }
    std::fs::write(
        d.path().join(format!("playerdata/{PLAYER_UUID}.dat")),
        compress::gzip_compress_stored(&nbt::write("", &p)),
    )
    .unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::PlayerData) > 0, "{}", dump(&w.report));
}

#[test]
fn truncated_player_file_detected() {
    let d = fixture_world("ptrunc", 2);
    let f = d.path().join(format!("playerdata/{PLAYER_UUID}.dat"));
    let data = std::fs::read(&f).unwrap();
    std::fs::write(&f, &data[..data.len() / 2]).unwrap();
    let w = world::load(d.path());
    assert!(errors_in(&w.report, Category::PlayerData) > 0, "{}", dump(&w.report));
}

// ------------------------------------------------------ item duplication

#[test]
fn item_duplication_detected_by_conservation() {
    let a = fixture_world("dup-a", 2);
    let b = TempDir::new("dup-b");
    copy_dir(a.path(), b.path());
    // Classic dupe: the chest still has its diamonds AND the player now has them too.
    let mut inv = default_inventory();
    inv.push(fixture::slot_item(2, "minecraft:diamond", 5));
    write_player(b.path(), inv);
    let (mut wa, mut wb) = (world::load(a.path()), world::load(b.path()));
    let mut r = Report::default();
    items::conserve(&wa.item_totals(), &wb.item_totals(), &[], false, &mut r);
    assert!(errors_in(&r, Category::ItemDuplication) > 0, "{}", dump(&r));
    assert!(r.issues.iter().any(|i| i.location == "minecraft:diamond"));
}

#[test]
fn item_move_between_container_and_player_is_conserved() {
    let a = fixture_world("move-a", 2);
    let b = TempDir::new("move-b");
    copy_dir(a.path(), b.path());
    // Move 5 diamonds chest -> player: totals unchanged.
    let c0 = fixture::chunk(0, 0, vec![fixture::slot_item(1, "minecraft:cobblestone", 64)]);
    let c1 = fixture::chunk(1, 0, vec![]);
    std::fs::write(b.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, c0), (1, 0, c1)], 2)).unwrap();
    let mut inv = default_inventory();
    inv.push(fixture::slot_item(2, "minecraft:diamond", 5));
    write_player(b.path(), inv);
    let (mut wa, mut wb) = (world::load(a.path()), world::load(b.path()));
    let mut r = Report::default();
    items::conserve(&wa.item_totals(), &wb.item_totals(), &[], true, &mut r);
    assert_eq!(r.errors(), 0, "{}", dump(&r));
}

#[test]
fn item_loss_is_warning_by_default_error_when_strict() {
    let a = fixture_world("loss-a", 2);
    let b = TempDir::new("loss-b");
    copy_dir(a.path(), b.path());
    let c0 = fixture::chunk(0, 0, vec![fixture::slot_item(0, "minecraft:diamond", 5)]); // cobblestone gone
    let c1 = fixture::chunk(1, 0, vec![]);
    std::fs::write(b.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, c0), (1, 0, c1)], 2)).unwrap();
    let (mut wa, mut wb) = (world::load(a.path()), world::load(b.path()));
    let (ta, tb) = (wa.item_totals(), wb.item_totals());
    let mut loose = Report::default();
    items::conserve(&ta, &tb, &[], false, &mut loose);
    assert!(loose.has(Category::ItemLoss, Severity::Warning));
    assert_eq!(loose.errors(), 0);
    let mut strict = Report::default();
    items::conserve(&ta, &tb, &[], true, &mut strict);
    assert!(strict.has(Category::ItemLoss, Severity::Error));
    // --allow suppresses an expected change (e.g. items consumed by a scenario step).
    let mut allowed = Report::default();
    items::conserve(&ta, &tb, &["minecraft:cobblestone".to_string()], true, &mut allowed);
    assert_eq!(allowed.errors(), 0);
}

#[test]
fn overstacked_item_detected() {
    let d = fixture_world("overstack", 2);
    let mut inv = default_inventory();
    inv.push(fixture::slot_item(3, "minecraft:diamond", 127));
    write_player(d.path(), inv);
    let mut w = world::load(d.path());
    let _ = w.item_totals();
    assert!(errors_in(&w.report, Category::ItemStack) > 0, "{}", dump(&w.report));
}

// ------------------------------------------------------- chunk loss / blocks

#[test]
fn chunk_loss_detected_by_diff() {
    let a = fixture_world("closs-a", 2);
    let b = TempDir::new("closs-b");
    copy_dir(a.path(), b.path());
    let c0 = fixture::chunk(0, 0, vec![fixture::slot_item(0, "minecraft:diamond", 5), fixture::slot_item(1, "minecraft:cobblestone", 64)]);
    std::fs::write(b.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, c0)], 2)).unwrap();
    let mut r = Report::default();
    world::diff(&world::load(a.path()), &world::load(b.path()), false, &mut r);
    assert_eq!(errors_in(&r, Category::ChunkLost), 1, "{}", dump(&r));
}

#[test]
fn block_change_across_reload_detected_in_strict_mode_only() {
    let a = fixture_world("blk-a", 2);
    let b = TempDir::new("blk-b");
    copy_dir(a.path(), b.path());
    let mut c0 = fixture::chunk(0, 0, vec![fixture::slot_item(0, "minecraft:diamond", 5), fixture::slot_item(1, "minecraft:cobblestone", 64)]);
    if let Tag::Compound(kv) = &mut c0 {
        for (k, v) in kv.iter_mut() {
            if k == "sections" {
                *v = Tag::List(10, vec![
                    fixture::section(-4, "minecraft:stone", "minecraft:deepslate"),
                    fixture::section(4, "minecraft:air", "minecraft:dirt"), // grass_block -> dirt
                ]);
            }
        }
    }
    let c1 = fixture::chunk(1, 0, vec![]);
    std::fs::write(b.path().join("region/r.0.0.mca"), anvil::write_region(&[(0, 0, c0), (1, 0, c1)], 2)).unwrap();
    let (wa, wb) = (world::load(a.path()), world::load(b.path()));
    let mut strict = Report::default();
    world::diff(&wa, &wb, true, &mut strict);
    assert!(errors_in(&strict, Category::BlockChanged) > 0, "{}", dump(&strict));
    assert!(strict.issues.iter().any(|i| i.message.contains("grass_block -> minecraft:dirt")), "{}", dump(&strict));
    let mut loose = Report::default();
    world::diff(&wa, &wb, false, &mut loose);
    assert_eq!(errors_in(&loose, Category::BlockChanged), 0);
    assert_eq!(loose.stats.get("diff_chunks_changed").copied(), Some(1));
}

#[test]
fn seed_change_detected() {
    let a = fixture_world("seed-a", 2);
    let b = TempDir::new("seed-b");
    copy_dir(a.path(), b.path());
    let lvl = Tag::Compound(vec![(
        "Data".into(),
        Tag::Compound(vec![
            ("DataVersion".into(), Tag::Int(0)),
            ("WorldGenSettings".into(), Tag::Compound(vec![("seed".into(), Tag::Long(42))])),
        ]),
    )]);
    std::fs::write(b.path().join("level.dat"), compress::gzip_compress_stored(&nbt::write("", &lvl))).unwrap();
    let mut r = Report::default();
    world::diff(&world::load(a.path()), &world::load(b.path()), false, &mut r);
    assert!(errors_in(&r, Category::LevelDat) > 0, "{}", dump(&r));
}

// ----------------------------------------------------------------- CLI

#[test]
fn cli_exit_codes() {
    let bin = env!("CARGO_BIN_EXE_world-verify");
    let d = TempDir::new("cli");
    let w = d.path().join("w");
    let st = std::process::Command::new(bin).args(["fixture", w.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success());
    let st = std::process::Command::new(bin).args(["verify", w.to_str().unwrap()]).output().unwrap();
    assert_eq!(st.status.code(), Some(0), "{}", String::from_utf8_lossy(&st.stdout));
    let json = d.path().join("r.json");
    let st = std::process::Command::new(bin)
        .args(["diff", w.to_str().unwrap(), w.to_str().unwrap(), "--strict", "--json", json.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(st.status.code(), Some(0));
    let j = std::fs::read_to_string(&json).unwrap();
    assert!(j.contains("\"command\""), "{j}");
    // Corrupt -> exit 1.
    let f = w.join("region/r.0.0.mca");
    let data = std::fs::read(&f).unwrap();
    std::fs::write(&f, &data[..2 * anvil::SECTOR + 10]).unwrap();
    let st = std::process::Command::new(bin).args(["verify", w.to_str().unwrap()]).output().unwrap();
    assert_eq!(st.status.code(), Some(1));
    // Missing dir -> exit 2.
    let st = std::process::Command::new(bin).args(["verify", d.path().join("nope").to_str().unwrap()]).output().unwrap();
    assert_eq!(st.status.code(), Some(2));
}
