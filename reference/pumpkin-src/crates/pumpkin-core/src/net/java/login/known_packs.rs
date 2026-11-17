#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_known_packs(&mut self, server: &Server) {
        let version = CURRENT_MC_VERSION;
        let mut synced = server.datapack_manager.get_all_synced_registries();
        if let Some(profile) = server.level_info.load().worldgen_profile.as_ref() {
            apply_worldgen_profile(&mut synced, profile);
        }
        for reg in &synced {
            self.send_packet_now(&CRegistryData::new(&reg.registry_id, &reg.registry_entries))
                .await;
        }
        let mut tags = Vec::new();
        for &key in pumpkin_data::tag::RegistryKey::NETWORK_KEYS {
            if pumpkin_data::tag::get_registry_key_tags(version, key)
                .is_some_and(|map| !map.is_empty())
            {
                tags.push(key);
            }
        }
        self.send_packet_now(&CUpdateTags::new(&tags)).await;
        self.send_packet_now(&CFinishConfig).await;
    }
}

fn apply_worldgen_profile(
    registries: &mut [pumpkin_data::registry::Registry],
    profile: &pumpkin_world::world_info::worldgen_profile::WorldgenProfile,
) {
    use crate::data::datapack::dynamic_registry_loader::json_to_registry_nbt_bytes;
    use pumpkin_data::registry::RegistryEntryData;

    for registry in registries {
        match registry.registry_id.as_str() {
            "minecraft:worldgen/biome" => {
                for &(id, json) in pumpkin_data::worldgen_profile::java_biomes() {
                    if let Ok(value) = serde_json::from_str(json) {
                        registry.registry_entries.push(RegistryEntryData {
                            entry_id: id.to_owned(),
                            data: Some(json_to_registry_nbt_bytes(&value)),
                        });
                    }
                }
            }
            "minecraft:dimension_type" => {
                for entry in &mut registry.registry_entries {
                    let height = match entry.entry_id.as_str() {
                        "minecraft:overworld" => profile.overworld,
                        "minecraft:the_nether" => profile.nether,
                        _ => continue,
                    };
                    if let Some(bytes) = entry.data.as_ref()
                        && let Ok(mut nbt) = pumpkin_nbt::Nbt::read_unnamed(
                            &mut pumpkin_nbt::deserializer::NbtReadHelperJava::new(
                                &mut std::io::Cursor::new(bytes.as_ref()),
                            ),
                        )
                    {
                        nbt.root_tag.put_int("min_y", height.min_y);
                        nbt.root_tag.put_int("height", height.height);
                        nbt.root_tag
                            .put_int("logical_height", height.logical_height);
                        entry.data = Some(nbt.write_unnamed().to_vec().into_boxed_slice());
                    }
                }
            }
            _ => {}
        }
    }
}
