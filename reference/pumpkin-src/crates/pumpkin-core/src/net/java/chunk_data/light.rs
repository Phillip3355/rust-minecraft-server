use pumpkin_protocol::codec::bit_set::BitSet;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::{CLightUpdate, LightData};
use pumpkin_protocol::ser::WritingError;
use pumpkin_util::version::JavaMinecraftVersion;
use pumpkin_world::chunk::ChunkData;
use pumpkin_world::chunk::format::LightContainer;

pub trait ChunkLightExt {
    fn from_chunk(chunk: &ChunkData, version: JavaMinecraftVersion) -> Result<Self, WritingError>
    where
        Self: Sized;
}

impl ChunkLightExt for CLightUpdate {
    fn from_chunk(chunk: &ChunkData, version: JavaMinecraftVersion) -> Result<Self, WritingError> {
        let light_data = light_data_from_chunk(chunk, version)?;
        Ok(Self {
            chunk_x: VarInt(chunk.x),
            chunk_z: VarInt(chunk.z),
            light_data,
        })
    }
}

#[expect(clippy::too_many_lines)]
pub fn light_data_from_chunk(
    chunk: &ChunkData,
    version: JavaMinecraftVersion,
) -> Result<LightData, WritingError> {
    let light_engine = chunk
        .light_engine
        .lock()
        .map_err(|_| WritingError::Message("light_engine lock poisoned".into()))?;

    if version < JavaMinecraftVersion::V_1_18 {
        let base_section = (0 - chunk.section.min_y).max(0) as usize / 16;
        let mut sky_light_mask = 0u64;
        let mut block_light_mask = 0u64;
        let mut sky_light_empty_mask = 0u64;
        let mut block_light_empty_mask = 0u64;
        let mut sky_light_arrays = Vec::new();
        let mut block_light_arrays = Vec::new();

        // Bit 0: Y = -1 (below world section 0)
        if base_section > 0 && base_section - 1 < light_engine.sky_light.len() {
            match &light_engine.sky_light[base_section - 1] {
                LightContainer::Full(data) => {
                    sky_light_mask |= 1 << 0;
                    sky_light_arrays.push(data.to_vec());
                }
                LightContainer::Empty(val) if *val > 0 => {
                    sky_light_mask |= 1 << 0;
                    sky_light_arrays.push(vec![*val << 4 | *val; 2048]);
                }
                LightContainer::Empty(_) => {
                    sky_light_empty_mask |= 1 << 0;
                }
            }
        } else {
            sky_light_empty_mask |= 1 << 0;
        }

        if base_section > 0 && base_section - 1 < light_engine.block_light.len() {
            match &light_engine.block_light[base_section - 1] {
                LightContainer::Full(data) => {
                    block_light_mask |= 1 << 0;
                    block_light_arrays.push(data.to_vec());
                }
                LightContainer::Empty(val) if *val > 0 => {
                    block_light_mask |= 1 << 0;
                    block_light_arrays.push(vec![*val << 4 | *val; 2048]);
                }
                LightContainer::Empty(_) => {
                    block_light_empty_mask |= 1 << 0;
                }
            }
        } else {
            block_light_empty_mask |= 1 << 0;
        }

        // Bits 1..=16: world sections (Y = 0..15)
        for i in 0..16 {
            let bit_index = i + 1;
            let sec_idx = base_section + i;

            if sec_idx < light_engine.sky_light.len() {
                match &light_engine.sky_light[sec_idx] {
                    LightContainer::Full(data) => {
                        sky_light_mask |= 1 << bit_index;
                        sky_light_arrays.push(data.to_vec());
                    }
                    LightContainer::Empty(val) if *val > 0 => {
                        sky_light_mask |= 1 << bit_index;
                        sky_light_arrays.push(vec![*val << 4 | *val; 2048]);
                    }
                    LightContainer::Empty(_) => {
                        sky_light_empty_mask |= 1 << bit_index;
                    }
                }
            } else {
                sky_light_empty_mask |= 1 << bit_index;
            }

            if sec_idx < light_engine.block_light.len() {
                match &light_engine.block_light[sec_idx] {
                    LightContainer::Full(data) => {
                        block_light_mask |= 1 << bit_index;
                        block_light_arrays.push(data.to_vec());
                    }
                    LightContainer::Empty(val) if *val > 0 => {
                        block_light_mask |= 1 << bit_index;
                        block_light_arrays.push(vec![*val << 4 | *val; 2048]);
                    }
                    LightContainer::Empty(_) => {
                        block_light_empty_mask |= 1 << bit_index;
                    }
                }
            } else {
                block_light_empty_mask |= 1 << bit_index;
            }
        }

        // Bit 17: Y = 16 (above world section 15)
        let top_sec = base_section + 16;
        if top_sec < light_engine.sky_light.len() {
            match &light_engine.sky_light[top_sec] {
                LightContainer::Full(data) => {
                    sky_light_mask |= 1 << 17;
                    sky_light_arrays.push(data.to_vec());
                }
                LightContainer::Empty(val) if *val > 0 => {
                    sky_light_mask |= 1 << 17;
                    sky_light_arrays.push(vec![*val << 4 | *val; 2048]);
                }
                LightContainer::Empty(_) => {
                    sky_light_empty_mask |= 1 << 17;
                }
            }
        } else {
            sky_light_empty_mask |= 1 << 17;
        }

        if top_sec < light_engine.block_light.len() {
            match &light_engine.block_light[top_sec] {
                LightContainer::Full(data) => {
                    block_light_mask |= 1 << 17;
                    block_light_arrays.push(data.to_vec());
                }
                LightContainer::Empty(val) if *val > 0 => {
                    block_light_mask |= 1 << 17;
                    block_light_arrays.push(vec![*val << 4 | *val; 2048]);
                }
                LightContainer::Empty(_) => {
                    block_light_empty_mask |= 1 << 17;
                }
            }
        } else {
            block_light_empty_mask |= 1 << 17;
        }

        Ok(LightData {
            trust_edges: true,
            sky_light_mask: BitSet::from_u64(sky_light_mask),
            block_light_mask: BitSet::from_u64(block_light_mask),
            empty_sky_light_mask: BitSet::from_u64(sky_light_empty_mask),
            empty_block_light_mask: BitSet::from_u64(block_light_empty_mask),
            sky_light_arrays,
            block_light_arrays,
        })
    } else {
        let num_sections = chunk.section.count;
        let (sky_light_mask, empty_sky_light_mask, sky_light_arrays) =
            encode_light_layer(&light_engine.sky_light, num_sections);
        let (block_light_mask, empty_block_light_mask, block_light_arrays) =
            encode_light_layer(&light_engine.block_light, num_sections);
        Ok(LightData {
            trust_edges: true,
            sky_light_mask,
            block_light_mask,
            empty_sky_light_mask,
            empty_block_light_mask,
            sky_light_arrays,
            block_light_arrays,
        })
    }
}

fn encode_light_layer(
    sections: &[LightContainer],
    section_count: usize,
) -> (BitSet, BitSet, Vec<Vec<u8>>) {
    // The first and last bits are the sections outside the build limits.
    let mut mask = BitSet::from_longs(vec![0; (section_count + 2).div_ceil(64)]);
    let mut empty_mask = mask.clone();
    let mut arrays = Vec::new();
    empty_mask.set_bit(0, true);

    for index in 0..section_count {
        if let Some(data) = sections.get(index).and_then(LightContainer::packed_bytes) {
            mask.set_bit(index + 1, true);
            arrays.push(data.into_owned());
        } else {
            empty_mask.set_bit(index + 1, true);
        }
    }

    empty_mask.set_bit(section_count + 1, true);
    (mask, empty_mask, arrays)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_update_round_trips_section_63_and_upper_boundary() {
        let chunk = ChunkData::empty_with_sections(0, 0, 64, -512);
        let mut light = chunk.light_engine.lock().unwrap();
        light.sky_light = vec![LightContainer::Empty(0); 64].into_boxed_slice();
        light.block_light = light.sky_light.clone();
        light.sky_light[63] = LightContainer::Empty(15);
        light.block_light[63] = LightContainer::new_filled(7);
        drop(light);

        let version = JavaMinecraftVersion::V_26_3;
        let light = light_data_from_chunk(&chunk, version).unwrap();
        let mut bytes = Vec::new();
        light.write(&mut bytes, &version).unwrap();
        let decoded = LightData::read(&mut bytes.as_slice(), &version).unwrap();
        assert!(decoded.sky_light_mask.get_bit(64));
        assert!(decoded.block_light_mask.get_bit(64));
        assert!(decoded.empty_sky_light_mask.get_bit(65));
        assert!(decoded.empty_block_light_mask.get_bit(65));
        assert_eq!(
            decoded.sky_light_arrays,
            [vec![0xff; LightContainer::ARRAY_SIZE]]
        );
        assert_eq!(
            decoded.block_light_arrays,
            [vec![0x77; LightContainer::ARRAY_SIZE]]
        );
    }
}
