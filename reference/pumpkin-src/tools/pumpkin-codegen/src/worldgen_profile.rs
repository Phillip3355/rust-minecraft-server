use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
};

const PROFILE: &str = "terralith_incendium_extreme_v1";
type Resources = BTreeMap<String, Vec<u8>>;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub fn stage(vanilla: &Path, terralith: &Path, incendium: &Path, output: &Path) -> Result<()> {
    let inputs = [
        ("vanilla", vanilla.join("data").canonicalize()?),
        ("terralith", terralith.join("data").canonicalize()?),
        ("incendium", incendium.join("data").canonicalize()?),
    ];
    let parent = output
        .parent()
        .ok_or("Profile output requires a parent directory")?
        .canonicalize()?;
    let output = parent.join(output.file_name().ok_or("Profile output requires a name")?);
    if inputs.iter().any(|(_, source)| output.starts_with(source)) {
        return Err("Profile output must be outside the input data directories".into());
    }

    let mut resources = Resources::new();
    let mut input_hashes = BTreeMap::new();
    let mut collisions = BTreeMap::<String, Vec<&str>>::new();
    let mut owners = BTreeMap::new();
    for (name, root) in &inputs {
        let mut layer = Resources::new();
        read_resources(root, root, &mut layer)?;
        if layer.is_empty() {
            return Err(format!("No resources found under {}", root.display()).into());
        }
        input_hashes.insert(*name, hashes(&layer));
        for (path, mut bytes) in layer {
            if let Some(previous) = resources.get(&path) {
                let history = collisions
                    .entry(path.clone())
                    .or_insert_with(|| vec![owners[&path]]);
                history.push(*name);
                if path.split('/').nth(1) == Some("tags") && path.ends_with(".json") {
                    bytes = serde_json::to_vec(
                        &overlay_tag(
                            &serde_json::from_slice(previous)?,
                            serde_json::from_slice(&bytes)?,
                        )
                        .map_err(|error| format!("{path}: {error}"))?,
                    )?;
                }
            }
            owners.insert(path.clone(), *name);
            resources.insert(path, bytes);
        }
    }

    let overworld = parse_resource(&resources, "minecraft/dimension/overworld.json")?;
    resolve_dimension_roots(&resources, &overworld)?;
    let overworld_biomes = validate_climate_biomes(&resources, &overworld, "terralith")?;
    if overworld_biomes == 0 {
        return Err("Overworld climate contains no Terralith biome references".into());
    }
    let nether_dimension = parse_resource(&resources, "minecraft/dimension/the_nether.json")?;
    resolve_dimension_roots(&resources, &nether_dimension)?;
    let nether_biomes = validate_climate_biomes(&resources, &nether_dimension, "incendium")?;
    if nether_biomes == 0 {
        return Err("Nether climate contains no Incendium biome references".into());
    }
    let nether = parse_resource(&resources, "minecraft/dimension_type/the_nether.json")?;
    let nether_geometry = (
        nether["min_y"].as_i64(),
        nether["height"].as_i64(),
        nether["logical_height"].as_i64(),
    );
    if nether_geometry != (Some(0), Some(256), Some(192)) {
        return Err("Unexpected Incendium Nether dimension geometry".into());
    }

    let manifest = json!({
        "profile": PROFILE,
        "version": 1,
        "status": "codegen_input",
        "pack_order": inputs.map(|(name, _)| name),
        "overworld_target": {
            "min_y": -512, "height": 1024, "logical_height": 1024,
            "ground_y": -400, "mountain_height": 800, "peak_y": 400,
            "terralith_climate_biomes": overworld_biomes
        },
        "nether_target": {
            "min_y": 0, "height": 256, "logical_height": 192,
            "incendium_climate_biomes": nether_biomes
        },
        "input_sha256": input_hashes,
        "output_sha256": hashes(&resources),
        "collisions": collisions
    });

    fs::create_dir(&output)?;
    let write = || -> Result<()> {
        for (path, bytes) in &resources {
            let target = output.join("data").join(path);
            fs::create_dir_all(
                target
                    .parent()
                    .ok_or("Resource requires a parent directory")?,
            )?;
            fs::write(target, bytes)?;
        }
        fs::write(
            output.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Ok(())
    };
    let result = write();
    if result.is_err() {
        fs::remove_dir_all(&output)?;
    }
    result
}

fn read_resources(root: &Path, directory: &Path, resources: &mut Resources) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_dir() {
            read_resources(root, &path, resources)?;
        } else if kind.is_file() {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some("json" | "nbt" | "mcfunction") => {}
                _ => continue,
            }
            let key = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("Non-UTF-8 resource path")?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let bytes = fs::read(&path)?;
            if key.ends_with(".json") {
                let _: Value = serde_json::from_slice(&bytes)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
            }
            resources.insert(key, bytes);
        } else {
            return Err(format!("Unsupported resource file type: {}", path.display()).into());
        }
    }
    Ok(())
}

fn hashes(resources: &Resources) -> BTreeMap<String, String> {
    resources
        .iter()
        .map(|(path, bytes)| {
            (
                path.clone(),
                Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
            )
        })
        .collect()
}

fn overlay_tag(previous: &Value, mut next: Value) -> Result<Value> {
    if next["replace"].as_bool() == Some(true) {
        return Ok(next);
    }
    let mut merged = previous["values"]
        .as_array()
        .ok_or("Missing tag values")?
        .clone();
    merged.append(next["values"].as_array_mut().ok_or("Missing tag values")?);
    Ok(json!({"values": merged}))
}

fn parse_resource(resources: &Resources, path: &str) -> Result<Value> {
    let bytes = resources
        .get(path)
        .ok_or_else(|| format!("Missing profile resource {path}"))?;
    serde_json::from_slice(bytes).map_err(|error| format!("{path}: {error}").into())
}

fn resolve_dimension_roots(resources: &Resources, dimension: &Value) -> Result<()> {
    let dimension_type = dimension["type"]
        .as_str()
        .ok_or("Missing dimension type ID")?;
    let settings = dimension["generator"]["settings"]
        .as_str()
        .ok_or("Missing noise settings ID")?;
    parse_resource(resources, &resource_path("dimension_type", dimension_type))?;
    parse_resource(
        resources,
        &resource_path("worldgen/noise_settings", settings),
    )?;
    Ok(())
}

fn resource_path(registry: &str, id: &str) -> String {
    let (namespace, name) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("{namespace}/{registry}/{name}.json")
}

fn validate_climate_biomes(
    resources: &Resources,
    dimension: &Value,
    custom_namespace: &str,
) -> Result<usize> {
    let biomes = dimension["generator"]["biome_source"]["biomes"]
        .as_array()
        .ok_or("Missing inline climate points")?;
    let mut custom_biomes = BTreeSet::new();
    for point in biomes {
        let id = point["biome"].as_str().ok_or("Missing climate biome ID")?;
        let resource = resource_path("worldgen/biome", id);
        if !resources.contains_key(&resource) {
            return Err(format!("Climate source references missing biome {id}").into());
        }
        if id
            .split_once(':')
            .is_some_and(|(namespace, _)| namespace == custom_namespace)
        {
            custom_biomes.insert(id);
        }
    }
    Ok(custom_biomes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_overlays_append_until_replaced() {
        let vanilla = json!({"values": ["minecraft:plains"]});
        let terralith = json!({"values": ["terralith:alpine_highlands"]});
        let merged = overlay_tag(&vanilla, terralith).unwrap();
        assert_eq!(
            merged["values"],
            json!(["minecraft:plains", "terralith:alpine_highlands"])
        );
        let replaced = overlay_tag(
            &merged,
            json!({"replace": true, "values": ["minecraft:desert"]}),
        )
        .unwrap();
        assert_eq!(replaced["values"], json!(["minecraft:desert"]));
    }

    #[test]
    fn climate_biome_references_must_exist_in_the_overlay() {
        let mut resources = BTreeMap::new();
        let dimension = json!({"generator": {"biome_source": {
            "type": "minecraft:multi_noise",
            "biomes": [{"biome": "terralith:alpine_highlands", "parameters": {}}]
        }}});
        assert!(validate_climate_biomes(&resources, &dimension, "terralith").is_err());
        resources.insert(
            "terralith/worldgen/biome/alpine_highlands.json".to_owned(),
            b"{}".to_vec(),
        );
        assert_eq!(
            validate_climate_biomes(&resources, &dimension, "terralith").unwrap(),
            1
        );
    }
}
