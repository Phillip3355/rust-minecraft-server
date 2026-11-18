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
    registries: &mut Vec<pumpkin_data::registry::Registry>,
    profile: &pumpkin_world::world_info::worldgen_profile::WorldgenProfile,
) {
    use crate::data::datapack::dynamic_registry_loader::json_to_registry_nbt_bytes;
    use pumpkin_data::registry::RegistryEntryData;

    for registry in &mut *registries {
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
    for profile_registry in pumpkin_data::worldgen_profile::structure_registries() {
        let index = registries
            .iter()
            .position(|registry| registry.registry_id == profile_registry.java)
            .unwrap_or_else(|| {
                registries.push(pumpkin_data::registry::Registry {
                    registry_id: profile_registry.java.to_owned(),
                    registry_entries: Vec::new(),
                });
                registries.len() - 1
            });
        let entries = &mut registries[index].registry_entries;
        for &(id, json) in profile_registry.entries {
            let value = serde_json::from_str(json).expect("Compiled structure registry JSON");
            let data = Some(json_to_registry_nbt_bytes(&value));
            if let Some(existing) = entries.iter_mut().find(|entry| entry.entry_id == id) {
                existing.data = data;
            } else {
                entries.push(RegistryEntryData {
                    entry_id: id.to_owned(),
                    data,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn profile_jigsaw_entries_are_encoded_for_configuration() {
        use pumpkin_data::worldgen_profile;
        let Some(terrain) = worldgen_profile::terrain_profile() else {
            return;
        };
        let profile =
            pumpkin_world::world_info::worldgen_profile::WorldgenProfile::from_compiled(terrain);
        let mut registries =
            pumpkin_data::registry::Registry::get_synced(super::CURRENT_MC_VERSION);
        super::apply_worldgen_profile(&mut registries, &profile);
        let structures = registries
            .iter()
            .find(|r| r.registry_id == "minecraft:worldgen/structure")
            .unwrap();
        for expected in worldgen_profile::structure_names() {
            let entry = structures
                .registry_entries
                .iter()
                .find(|entry| &entry.entry_id == expected)
                .unwrap();
            let mut cursor = std::io::Cursor::new(entry.data.as_deref().unwrap());
            let mut reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(&mut cursor);
            let nbt = pumpkin_nbt::Nbt::read_unnamed(&mut reader).unwrap();
            assert_eq!(nbt.root_tag.get_string("type"), Some("minecraft:jigsaw"));
            assert!(nbt.root_tag.get_string("start_pool").is_some());
        }
    }
}
