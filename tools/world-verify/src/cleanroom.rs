use std::collections::BTreeMap;

use crate::{
    anvil::section_blocks,
    nbt::Tag,
    report::{Category, Report},
    world::World,
};

const MIN_Y: i64 = -512;
const MAX_Y: i64 = 511;

#[derive(Default)]
struct Distribution {
    by_y: BTreeMap<i64, i64>,
}

impl Distribution {
    fn push(&mut self, y: i64) {
        *self.by_y.entry(y).or_default() += 1;
    }

    fn record(&self, report: &mut Report, name: &str, target: std::ops::RangeInclusive<i64>) {
        let count = self.by_y.values().sum::<i64>();
        let in_range = self.by_y.range(target).map(|(_, n)| n).sum::<i64>();
        report.bump(&format!("{name}_blocks"), count);
        report.bump(&format!("{name}_target_blocks"), in_range);
        if let (Some((&min, _)), Some((&max, _))) =
            (self.by_y.first_key_value(), self.by_y.last_key_value())
        {
            report.bump(&format!("{name}_min_y"), min);
            report.bump(&format!("{name}_max_y"), max);
        }
        if in_range == 0 {
            report.error(
                Category::ChunkStructure,
                "cleanroom",
                format!("no sampled {name} in target range"),
            );
        }
    }
}

pub fn verify(world: &World, report: &mut Report) {
    let Some(chunks) = world
        .terrain
        .get("dimensions/minecraft/overworld")
        .or_else(|| world.terrain.get("overworld"))
    else {
        report.error(Category::ChunkStructure, "cleanroom", "no Overworld chunks");
        return;
    };
    report.bump("cleanroom_min_y", MIN_Y);
    report.bump("cleanroom_max_y", MAX_Y);
    let mut caves = Distribution::default();
    let mut lava = Distribution::default();
    let mut deepslate = Distribution::default();
    let mut ore = Distribution::default();
    let mut plains = Vec::new();
    let mut peaks = Vec::new();
    let mut sea_floors = Vec::new();
    for (&(cx, cz), chunk) in chunks {
        let location = format!("cleanroom chunk({cx},{cz})");
        let Some(sections) = chunk.nbt.get("sections").and_then(Tag::as_list) else {
            continue; // The structural verifier reports invalid section lists.
        };
        let mut terrain = BTreeMap::new();
        for section in sections {
            let Some(y) = section.get("Y").and_then(Tag::as_i64) else {
                continue;
            };
            if let Some(blocks) = section_blocks(section) {
                terrain.insert(y, blocks);
            }
        }
        if terrain.keys().copied().ne((MIN_Y >> 4)..=(MAX_Y >> 4))
            || chunk.nbt.get("yPos").and_then(Tag::as_i64) != Some(MIN_Y >> 4)
        {
            report.error(
                Category::ChunkStructure,
                &location,
                "expected 64 block sections spanning Y=-512..511",
            );
            continue;
        }
        let mut tops = [MIN_Y; 256];
        let mut has_terrain = [false; 256];
        for (&section_y, (palette, indices)) in terrain.iter().rev() {
            let names: Vec<_> = palette
                .iter()
                .map(|entry| entry.get("Name").and_then(Tag::as_str).unwrap_or(""))
                .collect();
            for (index, &id) in indices.iter().enumerate().rev() {
                let y = section_y * 16 + (index / 256) as i64;
                let col = index % 256;
                let name = names[id as usize].trim_start_matches("minecraft:");
                match name {
                    "air" | "cave_air" | "void_air" => {
                        if has_terrain[col] && y <= tops[col] - 8 {
                            caves.push(y);
                        }
                    }
                    "water" => {}
                    "lava" => lava.push(y),
                    other => {
                        if matches!(
                            other,
                            "stone"
                                | "dirt"
                                | "grass_block"
                                | "deepslate"
                                | "sand"
                                | "gravel"
                                | "sandstone"
                                | "clay"
                                | "mud"
                                | "snow_block"
                                | "packed_ice"
                                | "terracotta"
                                | "tuff"
                                | "bedrock"
                        ) && !has_terrain[col]
                        {
                            has_terrain[col] = true;
                            tops[col] = y;
                        }
                        if other == "deepslate" {
                            deepslate.push(y);
                        }
                        if other.ends_with("_ore") {
                            ore.push(y);
                        }
                    }
                }
            }
        }
        tops.sort_unstable();
        let median = tops[128];
        if (-425..=-375).contains(&median) {
            plains.push(median);
        }
        if (300..=400).contains(&tops[255]) {
            peaks.push(tops[255]);
        }
        if (-485..=-445).contains(&median) {
            sea_floors.push(median);
        }
        report.bump("cleanroom_sample_chunks", 1);
        report.bump(&format!("sample_{cx}_{cz}_median_y"), median);
        report.bump(&format!("sample_{cx}_{cz}_max_terrain_y"), tops[255]);
    }
    for (label, heights) in [
        ("plain", plains),
        ("peak", peaks),
        ("deep_sea_floor", sea_floors),
    ] {
        report.bump(&format!("{label}_samples"), heights.len() as i64);
        if heights.is_empty() {
            report.error(
                Category::ChunkStructure,
                "cleanroom",
                format!("no sampled {label} in target range"),
            );
        }
    }
    caves.record(report, "caves", -507..=-420);
    lava.record(report, "lava", -511..=-458);
    deepslate.record(report, "deepslate", -511..=-444);
    ore.record(report, "ore", -508..=-390);
}
