use pumpkin_data::{
    dimension::Dimension,
    worldgen_profile::{self, TerrainProfile},
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::world_seed::Seed;
use serde::{Deserialize, Serialize};

use super::{LevelData, WorldInfoError};
use crate::generation::generator::{GeneratorInit, VanillaGenerator, WorldGenerator};

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

impl WorldgenProfile {
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
            if id != EXTREME_PROFILE_ID
                || version != 1
                || hash.len() != 64
                || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return None;
            }
            let overworld = parse_height("overworld")?;
            let nether = parse_height("nether")?;
            if (overworld.min_y, overworld.height, overworld.logical_height) != (-512, 1024, 1024)
                || (nether.min_y, nether.height, nether.logical_height) != (0, 256, 192)
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
