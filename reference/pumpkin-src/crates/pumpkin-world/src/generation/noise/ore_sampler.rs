use pumpkin_data::{Block, BlockState};
use pumpkin_util::{
    math::{clamped_map, vector3::Vector3},
    random::{RandomImpl, xoroshiro128::XoroshiroSplitter},
};

use crate::generation::noise::{VeinSample, router::chunk_noise_router::ChunkNoiseRouter};

pub struct OreVeinSampler {
    ranges: [(i32, i32); 2],
}

pub const VANILLA_VEIN_RANGES: [(i32, i32); 2] = [(-60, -8), (0, 50)];

impl OreVeinSampler {
    pub const fn new(ranges: [(i32, i32); 2]) -> Self {
        Self { ranges }
    }

    pub fn set_ranges(&mut self, ranges: [(i32, i32); 2]) {
        self.ranges = ranges;
    }

    #[must_use]
    pub fn sample(
        &self,
        router: &mut ChunkNoiseRouter,
        ore_random_deriver: &XoroshiroSplitter,
        pos: &Vector3<i32>,
        veins: &VeinSample,
    ) -> Option<&'static BlockState> {
        let vein_toggle = veins.toggle;
        let vein_type: &VeinType = if vein_toggle > 0.0 {
            &vein_type::COPPER
        } else {
            &vein_type::IRON
        };

        let block_y = pos.y;
        let (min_y, max_y) = self.ranges[usize::from(vein_toggle > 0.0)];
        let max_to_y = max_y - block_y;
        let y_to_min = block_y - min_y;
        if (max_to_y >= 0) && (y_to_min >= 0) {
            let closest_to_bound = max_to_y.min(y_to_min);
            let mapped_diff = clamped_map(closest_to_bound as f32, 0.0, 20.0, -0.2, 0.0);
            let abs_sample = vein_toggle.abs();
            if abs_sample + mapped_diff >= 0.4 {
                let mut random = ore_random_deriver.split_pos(pos.x, block_y, pos.z);

                let vein_ridged_sample = veins.ridged;
                if random.next_f32() <= 0.7 && vein_ridged_sample < 0.0 {
                    let clamped_sample = clamped_map(abs_sample, 0.4, 0.6, 0.1, 0.3);

                    let vein_gap = router.vein_gap(pos);
                    return if random.next_f32() < clamped_sample && vein_gap > -0.3 {
                        Some(if random.next_f32() < 0.02 {
                            vein_type.raw_ore.default_state
                        } else {
                            vein_type.ore.default_state
                        })
                    } else {
                        Some(vein_type.stone.default_state)
                    };
                }
            }
        }
        None
    }
}

pub struct VeinType {
    ore: Block,
    raw_ore: Block,
    stone: Block,
}

// One of the victims of removing compile time blocks
pub mod vein_type {
    use pumpkin_data::Block;

    use super::VeinType;
    pub const COPPER: VeinType = VeinType {
        ore: Block::COPPER_ORE,
        raw_ore: Block::RAW_COPPER_BLOCK,
        stone: Block::GRANITE,
    };
    pub const IRON: VeinType = VeinType {
        ore: Block::DEEPSLATE_IRON_ORE,
        raw_ore: Block::RAW_IRON_BLOCK,
        stone: Block::TUFF,
    };
    pub const MIN_Y: i32 = super::VANILLA_VEIN_RANGES[0].0;
    pub const MAX_Y: i32 = super::VANILLA_VEIN_RANGES[1].1;
}
