//! Synthetic world generator used by self-tests and the `fixture` command.
//!
//! The generated world follows the vanilla 1.18+ chunk layout. It is NOT a
//! Pumpkin-written world; it only exercises the verifier itself. Real-world
//! validation requires snapshots produced by the pinned Pumpkin binary.

use std::path::Path;

use crate::anvil;
use crate::compound;
use crate::compress;
use crate::nbt::{self, Tag};

/// Placeholder DataVersion for synthetic data only. The real 26.3 DataVersion
/// must be read from a Pumpkin/vanilla-written world, never assumed.
pub const FIXTURE_DATA_VERSION: i32 = 0;

pub const PLAYER_UUID: &str = "00000000-0000-4000-8000-000000000001";

fn s(v: &str) -> Tag {
    Tag::String(v.to_string())
}

fn block(name: &str) -> Tag {
    compound! { "Name" => s(name) }
}

pub fn item(id: &str, count: i32) -> Tag {
    compound! { "id" => s(id), "count" => Tag::Int(count) }
}

pub fn slot_item(slot: i8, id: &str, count: i32) -> Tag {
    compound! { "Slot" => Tag::Byte(slot), "id" => s(id), "count" => Tag::Int(count) }
}

/// Pack indices with `bits` per entry, non-spanning (1.16+).
pub fn pack(indices: &[u32], bits: u32) -> Vec<i64> {
    let per = (64 / bits) as usize;
    let mut out = vec![0i64; indices.len().div_ceil(per)];
    for (i, &v) in indices.iter().enumerate() {
        out[i / per] |= ((v as u64) << ((i % per) as u32 * bits)) as i64;
    }
    out
}

/// Section with stone below local y=8 and air above (2-entry palette).
pub fn section(y: i8, upper: &str, lower: &str) -> Tag {
    let idx: Vec<u32> = (0..4096).map(|i| if i / 256 < 8 { 1 } else { 0 }).collect();
    compound! {
        "Y" => Tag::Byte(y),
        "block_states" => compound! {
            "palette" => Tag::List(10, vec![block(upper), block(lower)]),
            "data" => Tag::LongArray(pack(&idx, 4)),
        },
        "biomes" => compound! { "palette" => Tag::List(8, vec![s("minecraft:plains")]) },
    }
}

pub fn chunk(cx: i32, cz: i32, chest_items: Vec<Tag>) -> Tag {
    let mut bes = Vec::new();
    if !chest_items.is_empty() {
        bes.push(compound! {
            "id" => s("minecraft:chest"),
            "x" => Tag::Int(cx * 16 + 1), "y" => Tag::Int(65), "z" => Tag::Int(cz * 16 + 1),
            "Items" => Tag::List(10, chest_items),
        });
    }
    compound! {
        "DataVersion" => Tag::Int(FIXTURE_DATA_VERSION),
        "xPos" => Tag::Int(cx), "zPos" => Tag::Int(cz), "yPos" => Tag::Int(-4),
        "Status" => s("minecraft:full"),
        "LastUpdate" => Tag::Long(100),
        "InhabitedTime" => Tag::Long(0),
        "sections" => Tag::List(10, vec![
            section(-4, "minecraft:stone", "minecraft:deepslate"),
            section(4, "minecraft:air", "minecraft:grass_block"),
        ]),
        "block_entities" => Tag::List(10, bes),
    }
}

pub fn player(inventory: Vec<Tag>) -> Tag {
    compound! {
        "DataVersion" => Tag::Int(FIXTURE_DATA_VERSION),
        "Pos" => Tag::List(6, vec![Tag::Double(1.5), Tag::Double(66.0), Tag::Double(1.5)]),
        "Health" => Tag::Float(20.0),
        "foodLevel" => Tag::Int(20),
        "XpLevel" => Tag::Int(3),
        "Dimension" => s("minecraft:overworld"),
        "Inventory" => Tag::List(10, inventory),
        "EnderItems" => Tag::List(10, vec![slot_item(0, "minecraft:ender_pearl", 4)]),
    }
}

pub fn level() -> Tag {
    compound! { "Data" => compound! {
        "DataVersion" => Tag::Int(FIXTURE_DATA_VERSION),
        "LevelName" => s("fixture"),
        "WorldGenSettings" => compound! { "seed" => Tag::Long(1_790_155_153_199_293_024) },
    } }
}

/// Write a small, valid world: overworld chunks (0,0),(1,0) with a chest,
/// one item entity, one player with inventory, using region compression `ctype`.
pub fn write_world(dir: &Path, ctype: u8) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("region"))?;
    std::fs::create_dir_all(dir.join("entities"))?;
    std::fs::create_dir_all(dir.join("playerdata"))?;
    std::fs::create_dir_all(dir.join("DIM-1/region"))?;
    std::fs::write(dir.join("level.dat"), compress::gzip_compress_stored(&nbt::write("", &level())))?;
    let chunks = vec![
        (0, 0, chunk(0, 0, vec![slot_item(0, "minecraft:diamond", 5), slot_item(1, "minecraft:cobblestone", 64)])),
        (1, 0, chunk(1, 0, vec![])),
    ];
    std::fs::write(dir.join("region/r.0.0.mca"), anvil::write_region(&chunks, ctype))?;
    std::fs::write(
        dir.join("DIM-1/region/r.-1.-1.mca"),
        anvil::write_region(&[(-1, -1, chunk(-1, -1, vec![]))], ctype),
    )?;
    let ent = compound! {
        "DataVersion" => Tag::Int(FIXTURE_DATA_VERSION),
        "Position" => Tag::IntArray(vec![0, 0]),
        "Entities" => Tag::List(10, vec![compound! {
            "id" => s("minecraft:item"),
            "Pos" => Tag::List(6, vec![Tag::Double(2.0), Tag::Double(66.0), Tag::Double(2.0)]),
            "Item" => item("minecraft:oak_log", 7),
        }]),
    };
    std::fs::write(dir.join("entities/r.0.0.mca"), anvil::write_region(&[(0, 0, ent)], ctype))?;
    let p = player(vec![
        slot_item(0, "minecraft:iron_pickaxe", 1),
        slot_item(1, "minecraft:bread", 12),
        slot_item(100, "minecraft:iron_boots", 1),
    ]);
    std::fs::write(
        dir.join(format!("playerdata/{PLAYER_UUID}.dat")),
        compress::gzip_compress_stored(&nbt::write("", &p)),
    )?;
    Ok(())
}
