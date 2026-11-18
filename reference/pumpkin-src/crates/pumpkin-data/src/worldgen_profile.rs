use crate::{
    biome::{Biome, ParameterPoint},
    dimension::Dimension,
    material_rule::MaterialRule,
    noise_router::BaseNoiseRouters,
    noise_settings::NoiseSettings,
    structures::{Structure, StructureKeys, StructureSet},
};

pub struct TerrainProfile {
    pub input_sha256: &'static str,
    pub overworld: ProfileDimension,
    pub nether: ProfileDimension,
}

pub struct StructureRegistry {
    pub java: &'static str,
    pub bedrock: &'static str,
    pub entries: &'static [(&'static str, &'static str)],
}

pub struct ProfileDimension {
    pub dimension: Dimension,
    pub router: &'static BaseNoiseRouters,
    pub climate: &'static [(ParameterPoint, &'static Biome)],
    pub settings: &'static NoiseSettings,
    pub surface: &'static MaterialRule,
    pub lava_y: i32,
    pub vein_y: (i32, i32),
}

#[cfg(pumpkin_compiled_worldgen_profile)]
mod compiled {
    include!(env!("PUMPKIN_WORLDGEN_PROFILE_SOURCE"));
}

#[cfg(not(pumpkin_compiled_worldgen_profile))]
mod compiled {
    use super::*;

    pub const STRUCTURE_REGISTRIES: &[StructureRegistry] = &[];
    pub const PROFILE_BIOMES: &[(&str, &str)] = &[];
    pub const PROFILE_STRUCTURE_NAMES: &[&str] = &[];
    pub const PROFILE_STRUCTURE_SETS: &[(&str, &StructureSet)] = &[];

    pub const fn generation_biome(_: u8) -> Option<&'static Biome> {
        None
    }
    pub const fn biome_by_id(_: u8) -> Option<&'static Biome> {
        None
    }
    pub fn biome_by_name(_: &str) -> Option<&'static Biome> {
        None
    }
    pub fn structure(_: &str) -> Option<(StructureKeys, &'static Structure, &'static [u16])> {
        None
    }
    pub fn template(_: &str) -> Option<&'static [u8]> {
        None
    }
    pub fn template_pool(_: &str) -> Option<&'static str> {
        None
    }
    pub fn processors(_: &str) -> Option<&'static str> {
        None
    }
}

pub use compiled::{
    biome_by_id, biome_by_name, generation_biome, processors, structure, template, template_pool,
};

pub fn terrain_profile() -> Option<&'static TerrainProfile> {
    #[cfg(pumpkin_compiled_worldgen_profile)]
    return Some(&compiled::TERRAIN_PROFILE);
    #[cfg(not(pumpkin_compiled_worldgen_profile))]
    None
}

pub fn java_biomes() -> &'static [(&'static str, &'static str)] {
    compiled::PROFILE_BIOMES
}

pub fn bedrock_biomes() -> Option<&'static [u8]> {
    #[cfg(pumpkin_compiled_worldgen_profile)]
    return Some(compiled::BEDROCK_BIOMES);
    #[cfg(not(pumpkin_compiled_worldgen_profile))]
    None
}

pub fn structure_names() -> &'static [&'static str] {
    compiled::PROFILE_STRUCTURE_NAMES
}

pub fn structure_sets() -> &'static [(&'static str, &'static StructureSet)] {
    compiled::PROFILE_STRUCTURE_SETS
}

pub fn structure_registries() -> &'static [StructureRegistry] {
    compiled::STRUCTURE_REGISTRIES
}
