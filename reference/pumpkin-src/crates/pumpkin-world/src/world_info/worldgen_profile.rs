use std::{fs, io::ErrorKind, path::Path};

pub use pumpkin_data::worldgen_profile::CLEANROOM_PROFILE_ID;

use pumpkin_data::{
    dimension::Dimension,
    worldgen_profile::{self, TerrainProfile},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::world_seed::Seed;
use serde::{Deserialize, Serialize};

use super::{LevelData, WorldInfoError};
use crate::generation::generator::{GeneratorInit, VanillaGenerator, WorldGenerator};

pub const CLEANROOM_METADATA_FILE: &str = "pumpkin_worldgen_profile.json";
use pumpkin_data::worldgen_profile::CLEANROOM_SETTINGS_REFERENCE;

pub const EXTREME_PROFILE_ID: &str = "terralith_incendium_extreme_v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHeight {
    pub min_y: i32,
    pub height: i32,
    pub logical_height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldgenProfile {
    pub id: String,
    pub version: i32,
    pub input_sha256: String,
    pub overworld: ProfileHeight,
    pub nether: ProfileHeight,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanroomMetadata {
    profile: String,
    version: i32,
    overworld: CleanroomOverworld,
    definition_sha256: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanroomOverworld {
    min_y: i32,
    height: i32,
    logical_height: i32,
    sea_level: i32,
    noise_settings: String,
}

impl WorldgenProfile {
    pub fn from_cleanroom(profile: &worldgen_profile::CleanroomProfile) -> Self {
        Self {
            id: CLEANROOM_PROFILE_ID.to_owned(),
            version: 1,
            input_sha256: profile.definition_sha256.to_owned(),
            overworld: ProfileHeight {
                min_y: profile.dimension.min_y,
                height: profile.dimension.height,
                logical_height: profile.dimension.logical_height,
            },
            nether: ProfileHeight {
                min_y: Dimension::THE_NETHER.min_y,
                height: Dimension::THE_NETHER.height,
                logical_height: Dimension::THE_NETHER.logical_height,
            },
        }
    }

    pub fn cleanroom() -> Result<Self, WorldInfoError> {
        worldgen_profile::cleanroom_profile()
            .map(Self::from_cleanroom)
            .ok_or_else(|| {
                WorldInfoError::UnavailableWorldgenProfile(CLEANROOM_PROFILE_ID.to_owned())
            })
    }

    fn cleanroom_metadata(&self) -> CleanroomMetadata {
        CleanroomMetadata {
            profile: self.id.clone(),
            version: self.version,
            overworld: CleanroomOverworld {
                min_y: self.overworld.min_y,
                height: self.overworld.height,
                logical_height: self.overworld.logical_height,
                sea_level: worldgen_profile::cleanroom_profile()
                    .map_or(-400, |profile| profile.settings.sea_level),
                noise_settings: CLEANROOM_SETTINGS_REFERENCE.to_owned(),
            },
            definition_sha256: self.input_sha256.clone(),
        }
    }

    fn has_cleanroom_generator(settings: &super::WorldGenSettings) -> bool {
        settings
            .dimensions
            .get("minecraft:overworld")
            .is_some_and(|overworld| {
                overworld.dimension_type == "minecraft:overworld"
                    && overworld.generator.generator_type == "minecraft:noise"
                    && matches!(&overworld.generator.settings,
                    Some(super::GeneratorSettings::Reference(reference))
                        if reference == CLEANROOM_SETTINGS_REFERENCE)
            })
    }

    pub fn validate_world(
        root: &Path,
        level: Option<&LevelData>,
        expected: Option<&Self>,
    ) -> Result<(), WorldInfoError> {
        let metadata = match fs::read(root.join(CLEANROOM_METADATA_FILE)) {
            Ok(bytes) => Some(
                serde_json::from_slice::<CleanroomMetadata>(&bytes)
                    .map_err(|error| WorldInfoError::DeserializationError(error.to_string()))?,
            ),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let expected_metadata = expected.map(Self::cleanroom_metadata);
        if let Some(level) = level {
            level.worldgen_profile_matches(expected)?;
            if metadata != expected_metadata
                || (expected.is_some() && !Self::has_cleanroom_generator(&level.world_gen_settings))
            {
                return Err(WorldInfoError::WorldgenProfileMismatch);
            }
        } else if expected.is_some() {
            if metadata.is_some() && metadata != expected_metadata {
                return Err(WorldInfoError::WorldgenProfileMismatch);
            }
            for relative in [
                "level.dat",
                "level.dat_old",
                "region",
                "DIM-1",
                "DIM1",
                "dimensions",
            ] {
                if root.join(relative).try_exists()? {
                    return Err(WorldInfoError::WorldgenProfileMismatch);
                }
            }
            // The matching sidecar can survive an interrupted first write. Recover
            // only while there is no evidence the world ever reached chunk I/O.
            if root
                .join("data/minecraft/world_gen_settings.dat")
                .try_exists()?
                && !(metadata.is_some()
                    && super::data_files::read_world_gen_settings(root)
                        .as_ref()
                        .is_some_and(Self::has_cleanroom_generator))
            {
                return Err(WorldInfoError::WorldgenProfileMismatch);
            }
        } else if metadata.is_some() {
            return Err(WorldInfoError::WorldgenProfileMismatch);
        }
        Ok(())
    }

    pub fn initialize_cleanroom(
        &self,
        root: &Path,
        seed: Seed,
    ) -> Result<LevelData, WorldInfoError> {
        use super::{WorldInfoWriter, anvil::AnvilLevelInfo};
        let compiled = worldgen_profile::cleanroom_profile()
            .ok_or_else(|| WorldInfoError::UnavailableWorldgenProfile(self.id.clone()))?;
        if *self != Self::from_cleanroom(compiled) {
            return Err(WorldInfoError::WorldgenProfileMismatch);
        }
        Self::validate_world(root, None, Some(self))?;
        fs::create_dir_all(root)?;
        let path = root.join(CLEANROOM_METADATA_FILE);
        if !path.try_exists()? {
            let mut metadata = tempfile::NamedTempFile::new_in(root)?;
            serde_json::to_writer_pretty(&mut metadata, &self.cleanroom_metadata())
                .map_err(|error| WorldInfoError::SerializationError(error.to_string()))?;
            metadata.as_file().sync_all()?;
            match metadata.persist_noclobber(&path) {
                Ok(_) => {}
                Err(error) if error.error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.error.into()),
            }
        }
        let lock = fs::OpenOptions::new().read(true).write(true).open(path)?;
        lock.try_lock().map_err(std::io::Error::from)?;
        Self::validate_world(root, None, Some(self))?;
        let seed = super::data_files::read_world_gen_settings(root)
            .map_or(seed, |settings| Seed(settings.seed as u64));
        let generator = VanillaGenerator::from_cleanroom(seed, compiled);
        let mut level = LevelData::from_world_generator(seed, &generator);
        level.spawn_y = compiled.settings.sea_level + 1;
        let overworld = level
            .world_gen_settings
            .dimensions
            .get_mut("minecraft:overworld")
            .expect("Normal world settings include the overworld");
        overworld.generator.settings = Some(super::GeneratorSettings::Reference(
            CLEANROOM_SETTINGS_REFERENCE.to_owned(),
        ));
        level.worldgen_profile = Some(self.clone());

        AnvilLevelInfo.write_world_info(&level, root)?;
        Ok(level)
    }

    pub fn from_compiled(profile: &TerrainProfile) -> Self {
        let height = |dimension: &pumpkin_data::dimension::Dimension| ProfileHeight {
            min_y: dimension.min_y,
            height: dimension.height,
            logical_height: dimension.logical_height,
        };
        Self {
            id: EXTREME_PROFILE_ID.to_owned(),
            version: 1,
            input_sha256: profile.input_sha256.to_owned(),
            overworld: height(&profile.overworld.dimension),
            nether: height(&profile.nether.dimension),
        }
    }

    pub fn generator(
        &self,
        seed: Seed,
        dimension: Dimension,
    ) -> Result<WorldGenerator, WorldInfoError> {
        if self.id == CLEANROOM_PROFILE_ID {
            let compiled = worldgen_profile::cleanroom_profile()
                .ok_or_else(|| WorldInfoError::UnavailableWorldgenProfile(self.id.clone()))?;
            if *self != Self::from_cleanroom(compiled) {
                return Err(WorldInfoError::WorldgenProfileMismatch);
            }
            let generator = if dimension == Dimension::OVERWORLD {
                VanillaGenerator::from_cleanroom(seed, compiled)
            } else {
                VanillaGenerator::new(seed, dimension)
            };
            return Ok(WorldGenerator::Noise(Box::new(generator)));
        }
        let compiled = worldgen_profile::terrain_profile()
            .ok_or_else(|| WorldInfoError::UnavailableWorldgenProfile(self.id.clone()))?;
        if *self != Self::from_compiled(compiled) {
            return Err(WorldInfoError::WorldgenProfileMismatch);
        }
        let generator = if dimension == Dimension::OVERWORLD {
            VanillaGenerator::from_profile(seed, &compiled.overworld)
        } else if dimension == Dimension::THE_NETHER {
            VanillaGenerator::from_profile(seed, &compiled.nether)
        } else {
            VanillaGenerator::new(seed, dimension)
        };
        Ok(WorldGenerator::Noise(Box::new(generator)))
    }

    pub(crate) fn to_nbt(&self) -> NbtCompound {
        let mut tag = NbtCompound::new();
        tag.put_string("id", self.id.clone());
        tag.put_int("version", self.version);
        tag.put_string("input_sha256", self.input_sha256.clone());
        for (name, height) in [("overworld", self.overworld), ("nether", self.nether)] {
            let mut value = NbtCompound::new();
            value.put_int("min_y", height.min_y);
            value.put_int("height", height.height);
            value.put_int("logical_height", height.logical_height);
            tag.put_compound(name, value);
        }
        tag
    }

    pub(crate) fn from_nbt(tag: &NbtCompound) -> Result<Self, WorldInfoError> {
        let parse_height = |name| {
            let value = tag.get_compound(name)?;
            Some(ProfileHeight {
                min_y: value.get_int("min_y")?,
                height: value.get_int("height")?,
                logical_height: value.get_int("logical_height")?,
            })
        };
        let parse = || {
            let id = tag.get_string("id")?;
            let version = tag.get_int("version")?;
            let hash = tag.get_string("input_sha256")?;
            if !matches!(id, EXTREME_PROFILE_ID | CLEANROOM_PROFILE_ID)
                || version != 1
                || hash.len() != 64
                || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            let overworld = parse_height("overworld")?;
            let nether = parse_height("nether")?;
            let expected_nether = if id == CLEANROOM_PROFILE_ID {
                (0, 256, 128)
            } else {
                (0, 256, 192)
            };
            if (overworld.min_y, overworld.height, overworld.logical_height) != (-512, 1024, 1024)
                || (nether.min_y, nether.height, nether.logical_height) != expected_nether
            {
                return None;
            }
            Some(Self {
                id: id.to_owned(),
                version,
                input_sha256: hash.to_owned(),
                overworld,
                nether,
            })
        };
        parse().ok_or_else(|| {
            WorldInfoError::DeserializationError("Invalid PumpkinWorldgenProfile tag".into())
        })
    }
}

impl LevelData {
    pub fn worldgen_profile_matches(
        &self,
        expected: Option<&WorldgenProfile>,
    ) -> Result<(), WorldInfoError> {
        if self.worldgen_profile.as_ref() == expected {
            Ok(())
        } else {
            Err(WorldInfoError::WorldgenProfileMismatch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> WorldgenProfile {
        WorldgenProfile {
            id: EXTREME_PROFILE_ID.to_owned(),
            version: 1,
            input_sha256: "42".repeat(32),
            overworld: ProfileHeight {
                min_y: -512,
                height: 1024,
                logical_height: 1024,
            },
            nether: ProfileHeight {
                min_y: 0,
                height: 256,
                logical_height: 192,
            },
        }
    }

    #[test]
    fn profile_metadata_round_trips_without_a_compiled_generator() {
        let profile = identity();
        assert_eq!(
            WorldgenProfile::from_nbt(&profile.to_nbt()).unwrap(),
            profile
        );
    }

    #[test]
    fn profile_metadata_rejects_corrupt_dimension_bounds() {
        let mut profile = identity();
        profile.overworld.min_y = -64;
        assert!(matches!(
            WorldgenProfile::from_nbt(&profile.to_nbt()),
            Err(WorldInfoError::DeserializationError(_))
        ));
    }
}
