//! What the pinned Pumpkin source understands, extracted from the source tree.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::rustsrc::{enum_body, first_literal_on_lines, fn_body, literals_after, match_arm_literals, serde_names, string_literals};

pub type Set = BTreeSet<String>;

/// Normalise a resource location to `namespace:path` (drops a leading `#`).
pub fn ns(s: &str) -> String {
    let s = s.trim_start_matches('#');
    if s.contains(':') {
        s.to_string()
    } else {
        format!("minecraft:{s}")
    }
}

#[derive(Default, Debug)]
pub struct Capabilities {
    /// category -> namespaced type ids Pumpkin handles
    pub types: BTreeMap<String, Set>,
    /// category -> `file::item` the set was read from
    pub sources: BTreeMap<String, Vec<String>>,
    /// registry dir -> compiled-in vanilla ids
    pub vanilla_ids: BTreeMap<String, Set>,
    /// datapack dir (e.g. `worldgen/biome`) -> registry id from DYNAMIC_REGISTRIES
    pub dynamic_registry_dirs: BTreeMap<String, String>,
    pub excluded_registry_dirs: Set,
    /// registries in Pumpkin's base synced set (sent to the client by default)
    pub synced_registries: Set,
    pub commands: Set,
}

impl Capabilities {
    pub fn has(&self, cat: &str, t: &str) -> bool {
        self.types.get(cat).is_some_and(|s| s.contains(t))
    }
    pub fn vanilla(&self, reg: &str, id: &str) -> bool {
        self.vanilla_ids.get(reg).is_some_and(|s| s.contains(id))
    }
}

fn read(p: &Path) -> Result<String, String> {
    fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

pub fn load(src: &Path) -> Result<Capabilities, String> {
    let mut cap = Capabilities::default();
    let cg = src.join("tools/pumpkin-codegen/src");
    let core = src.join("crates/pumpkin-core/src");
    let world = src.join("crates/pumpkin-world/src");
    let gen = src.join("crates/pumpkin-data/src/generated");
    let util = src.join("crates/pumpkin-util/src");

    let rel = |p: &Path| p.strip_prefix(src).unwrap_or(p).display().to_string();

    // Match-arm based dispatchers. `add_ns` = arms are written without the
    // `minecraft:` prefix; otherwise only `minecraft:` literals are kept (drops
    // unrelated nested matches such as heightmap names).
    let arms = |cap: &mut Capabilities, cat: &str, file: &Path, func: &str, add_ns: bool, drop: &[&str]| -> Result<(), String> {
        let text = read(file)?;
        let body = fn_body(&text, func).ok_or_else(|| format!("fn {func} not found in {}", rel(file)))?;
        let set = cap.types.entry(cat.to_string()).or_default();
        for lit in match_arm_literals(body) {
            if drop.contains(&lit.as_str()) {
                continue;
            }
            if add_ns {
                if !lit.is_empty() {
                    set.insert(ns(&lit));
                }
            } else if lit.starts_with("minecraft:") {
                set.insert(lit);
            }
        }
        cap.sources.entry(cat.to_string()).or_default().push(format!("{}::{func}", rel(file)));
        Ok(())
    };
    let senum = |cap: &mut Capabilities, cat: &str, file: &Path, name: &str| -> Result<(), String> {
        let text = read(file)?;
        let body = enum_body(&text, name).ok_or_else(|| format!("enum {name} not found in {}", rel(file)))?;
        let set = cap.types.entry(cat.to_string()).or_default();
        set.extend(serde_names(body).into_iter().filter(|n| n.starts_with("minecraft:")));
        cap.sources.entry(cat.to_string()).or_default().push(format!("{}::{name}", rel(file)));
        Ok(())
    };

    // Compile-time worldgen: codegen reads vanilla JSON with the same schema a datapack uses.
    arms(&mut cap, "density_function", &cg.join("noise_router.rs"), "parse_vanilla_df", true, &[])?;
    let cf = cg.join("configured_feature.rs");
    let pf = cg.join("placed_feature.rs");
    arms(&mut cap, "feature", &cf, "value_to_configured_feature", false, &[])?;
    arms(&mut cap, "block_state_provider", &cf, "value_to_block_state_provider", false, &[])?;
    arms(&mut cap, "trunk_placer", &cf, "value_to_trunk_placer", false, &[])?;
    arms(&mut cap, "foliage_placer", &cf, "value_to_foliage_placer", false, &[])?;
    arms(&mut cap, "tree_decorator", &cf, "value_to_tree_decorator", false, &[])?;
    arms(&mut cap, "root_placer", &cf, "value_to_root_placer", false, &[])?;
    arms(&mut cap, "feature_size", &cf, "value_to_feature_size", false, &[])?;
    arms(&mut cap, "rule_test", &cf, "value_to_rule_test", false, &[])?;
    arms(&mut cap, "placement_modifier", &pf, "value_to_placement_modifier", false, &[])?;
    arms(&mut cap, "placement_modifier", &cf, "value_to_placement_modifier_cf", false, &[])?;
    arms(&mut cap, "block_predicate", &pf, "value_to_block_predicate", false, &[])?;
    arms(&mut cap, "int_provider", &pf, "value_to_int_provider", false, &[])?;
    arms(&mut cap, "height_provider", &pf, "value_to_height_provider", false, &[])?;
    arms(&mut cap, "float_provider", &cg.join("carver.rs"), "value_to_float_provider", false, &[])?;
    arms(&mut cap, "carver", &cg.join("carver.rs"), "config", false, &[])?;
    // The carver config calls the provider parsers separately,
    // but its direct arms include provider names in some revisions; keep carver ids only.
    if let Some(s) = cap.types.get_mut("carver") {
        let providers: Set = ["minecraft:constant", "minecraft:uniform", "minecraft:trapezoid", "minecraft:biased_to_bottom", "minecraft:very_biased_to_bottom"].iter().map(|s| s.to_string()).collect();
        s.retain(|x| !providers.contains(x));
    }
    arms(&mut cap, "structure_type", &cg.join("structures.rs"), "structure_type_to_token", false, &[])?;
    senum(&mut cap, "structure_placement", &cg.join("structures.rs"), "StructurePlacementTypeStruct")?;
    senum(&mut cap, "surface_rule", &cg.join("material_rule.rs"), "DirectMaterialRule")?;
    senum(&mut cap, "surface_condition", &cg.join("material_rule.rs"), "DirectMaterialCondition")?;

    // Runtime (datapack-loaded) worldgen pieces.
    let proc_rs = world.join("generation/structure/template/processor.rs");
    senum(&mut cap, "rt_pool_element", &world.join("generation/structure/structures/jigsaw.rs"), "RawPoolElement")?;
    senum(&mut cap, "rt_processor", &proc_rs, "RawProcessor")?;
    senum(&mut cap, "rt_rule_test", &proc_rs, "RawRuleTest")?;
    senum(&mut cap, "rt_pos_rule_test", &proc_rs, "RawPosRuleTest")?;
    senum(&mut cap, "rt_block_entity_modifier", &proc_rs, "RawBlockEntityModifier")?;

    // Runtime gameplay data.
    arms(&mut cap, "recipe", &core.join("data/datapack/recipe_loader.rs"), "parse_recipe", true, &[])?;
    let lp = util.join("loot_table/parser.rs");
    arms(&mut cap, "loot_entry", &lp, "entry", true, &[])?;
    arms(&mut cap, "loot_condition", &lp, "condition", true, &[])?;
    arms(&mut cap, "loot_function", &lp, "functions", true, &["ore_drops", "uniform_bonus_count", "binomial_with_bonus_count"])?;
    arms(&mut cap, "loot_number_provider", &lp, "provider", true, &[])?;

    // Compiled-in vanilla ids.
    let from_name = |file: &Path| -> Result<Set, String> { Ok(first_literal_on_lines(&read(file)?, "=> Some(Self::").iter().map(|s| ns(s)).collect()) };
    cap.vanilla_ids.insert("worldgen/placed_feature".into(), from_name(&gen.join("placed_feature.rs"))?);
    cap.vanilla_ids.insert("worldgen/configured_feature".into(), from_name(&gen.join("configured_feature.rs"))?);
    cap.vanilla_ids.insert("worldgen/structure".into(), from_name(&gen.join("structures.rs"))?);
    cap.vanilla_ids.insert("worldgen/biome".into(), literals_after(&read(&gen.join("biome.rs"))?, "registry_id: \"").iter().map(|s| ns(s)).collect());
    let noise_text = read(&gen.join("noise_parameter.rs"))?;
    let noise_body = fn_body(&noise_text, "id_to_parameters").ok_or("id_to_parameters not found")?;
    cap.vanilla_ids.insert("worldgen/noise".into(), first_literal_on_lines(noise_body, "=> &Self::").iter().map(|s| ns(s)).collect());

    // Dynamic registry loader tables.
    let drl = read(&core.join("data/datapack/dynamic_registry_loader.rs"))?;
    let section = |name: &str| -> Option<String> {
        let s = drl.find(name)?;
        let e = s + drl[s..].find("];")?;
        Some(drl[s..e].to_string())
    };
    let mut current = String::new();
    for lit in string_literals(&section("DYNAMIC_REGISTRIES: &[").ok_or("DYNAMIC_REGISTRIES not found")?) {
        if lit.starts_with("minecraft:") {
            current = lit;
        } else if !current.is_empty() {
            cap.dynamic_registry_dirs.entry(lit).or_insert_with(|| current.clone());
        }
    }
    cap.excluded_registry_dirs = string_literals(&section("EXCLUDED_REGISTRY_DIRS").ok_or("EXCLUDED_REGISTRY_DIRS not found")?).into_iter().collect();
    cap.synced_registries = literals_after(&read(&gen.join("registry.rs"))?, "StaticRegistry { registry_id : \"").iter().map(|s| ns(s)).collect();

    // Command names registered by default_dispatcher().
    for e in fs::read_dir(core.join("command/commands")).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        let t = read(&p)?;
        cap.commands.extend(literals_after(&t, "command(\""));
        cap.commands.extend(literals_after(&t, "const NAME: &str = \""));
        for marker in ["const NAMES: [&str;", "register_with_aliases("] {
            let mut from = 0;
            while let Some(off) = t[from..].find(marker) {
                let at = from + off;
                let open = if marker.starts_with("const") { t[at..].find("= [") } else { t[at..].find("&[") };
                if let Some(o) = open {
                    let start = at + o;
                    if let Some(c) = t[start..].find(']') {
                        cap.commands.extend(string_literals(&t[start..start + c]));
                    }
                }
                from = at + marker.len();
            }
        }
    }
    cap.commands.retain(|c| !c.is_empty() && c.bytes().all(|b| b.is_ascii_lowercase() || b == b'-' || b == b'_'));
    Ok(cap)
}
