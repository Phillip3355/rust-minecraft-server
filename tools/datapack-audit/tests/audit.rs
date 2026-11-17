//! Regression tests. Capability tests read the vendored Pumpkin source
//! (`reference/pumpkin-src`, pinned eda69440); pack tests use small synthetic
//! datapacks written to a temp dir, each built to trigger one Pumpkin behaviour.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use datapack_audit::audit::{audit_pack, function_calls, reachable, Pack, PackResult};
use datapack_audit::caps::{self, Capabilities};

fn pumpkin_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/pumpkin-src")
}

fn cap() -> &'static Capabilities {
    static C: OnceLock<Capabilities> = OnceLock::new();
    C.get_or_init(|| caps::load(&pumpkin_src()).expect("capability extraction from vendored Pumpkin source"))
}

struct TempPack(PathBuf);
impl Drop for TempPack {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn pack(name: &str, files: &[(&str, &[u8])]) -> TempPack {
    let root = std::env::temp_dir().join(format!("datapack-audit-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("pack.mcmeta"), br#"{"pack":{"description":"t","pack_format":94}}"#).unwrap();
    for (rel, body) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }
    TempPack(root)
}

fn run(p: &TempPack) -> PackResult {
    audit_pack(&Pack::open(&p.0), cap())
}

fn has(r: &PackResult, sev: &str, cat: &str, item: &str) -> bool {
    r.findings.items.get(&(cat.to_string(), item.to_string())).is_some_and(|f| f.severity == sev)
}

// ---------------------------------------------------------------- capabilities

#[test]
fn extracts_density_function_types_from_codegen() {
    let df = &cap().types["density_function"];
    for t in ["noise", "shifted_noise", "spline", "interpolated", "flat_cache", "range_choice", "y_clamped_gradient", "old_blended_noise", "beardifier"] {
        assert!(df.contains(&format!("minecraft:{t}")), "missing {t}");
    }
    // Unrelated nested matches (axis names) must not leak in.
    assert!(!df.contains("minecraft:x"));
}

#[test]
fn extracts_runtime_structure_types() {
    let c = cap();
    assert_eq!(c.types["rt_pool_element"].len(), 5);
    for t in ["rule", "capped", "block_rot", "protected_blocks"] {
        assert!(c.has("rt_processor", &format!("minecraft:{t}")), "{t}");
    }
    assert!(c.has("structure_type", "minecraft:jigsaw"));
    assert!(c.has("carver", "minecraft:cave") && c.has("carver", "minecraft:canyon"));
    assert!(!c.has("carver", "minecraft:uniform"), "provider names must not count as carvers");
}

#[test]
fn extracts_compiled_vanilla_ids_and_registry_tables() {
    let c = cap();
    assert!(c.vanilla("worldgen/biome", "minecraft:plains"));
    assert!(c.vanilla("worldgen/structure", "minecraft:village_plains"));
    assert!(c.vanilla("worldgen/placed_feature", "minecraft:amethyst_geode"));
    assert!(c.vanilla("worldgen/noise", "minecraft:cave_cheese"));
    assert_eq!(c.dynamic_registry_dirs.get("worldgen/density_function").map(String::as_str), Some("minecraft:worldgen/density_function"));
    assert!(c.excluded_registry_dirs.contains("loot_table"));
    assert!(c.synced_registries.contains("minecraft:worldgen/biome"));
    assert!(!c.synced_registries.contains("minecraft:worldgen/density_function"));
}

#[test]
fn extracts_commands_including_aliases() {
    let c = cap();
    for n in ["function", "execute", "scoreboard", "tp", "xp", "msg", "tell", "schedule", "data", "item"] {
        assert!(c.commands.contains(n), "{n}");
    }
}

#[test]
fn loot_capabilities_match_parser() {
    let c = cap();
    assert!(c.has("loot_function", "minecraft:set_count"));
    assert!(!c.has("loot_function", "minecraft:ore_drops"), "apply_bonus formula is not a function");
    assert!(!c.has("loot_function", "minecraft:set_components"));
    assert!(c.has("loot_entry", "minecraft:item"));
}

// ---------------------------------------------------------------- pack checks

#[test]
fn flags_unknown_density_function_type_and_foreign_refs() {
    let p = pack("df", &[(
        "data/x/worldgen/density_function/a.json",
        br#"{"type":"minecraft:add","argument1":"x:other","argument2":{"type":"minecraft:made_up","argument":1}}"#,
    )]);
    let r = run(&p);
    assert!(has(&r, "FAIL", "density_function", "minecraft:made_up"));
    assert!(has(&r, "FAIL", "density_function-ref", "non-minecraft density function reference"));
    assert!(has(&r, "FAIL", "density_function-id", "custom density_function ids"));
    assert!(!r.findings.has("density_function", "minecraft:add"));
}

#[test]
fn inline_placed_feature_refers_to_configured_feature() {
    let p = pack("inline", &[(
        "data/minecraft/worldgen/configured_feature/ore_diamond_medium.json",
        br#"{"type":"minecraft:random_selector","config":{"default":{"feature":"minecraft:amethyst_geode","placement":[]},"features":[{"chance":0.5,"feature":"minecraft:amethyst_geode"}]}}"#,
    )]);
    let r = run(&p);
    // `amethyst_geode` exists both as configured and placed vanilla feature: no unresolved refs either way.
    assert!(!r.findings.items.keys().any(|(c, _)| c.ends_with("-ref")), "{:?}", r.findings.items.keys().collect::<Vec<_>>());
    let p = pack("inline2", &[(
        "data/minecraft/worldgen/configured_feature/ore_diamond_medium.json",
        br#"{"type":"minecraft:random_selector","config":{"default":{"feature":"minecraft:acacia_checked","placement":[]},"features":[]}}"#,
    )]);
    // acacia_checked is a placed-feature id only, so as a configured ref it is unresolved.
    assert!(run(&p).findings.has("worldgen/configured_feature-ref", "unresolved reference (not compiled, not in this pack)"));
}

#[test]
fn template_pool_with_unknown_element_is_rejected() {
    let p = pack("pool", &[
        ("data/x/worldgen/template_pool/ok.json", br#"{"fallback":"minecraft:empty","elements":[{"weight":1,"element":{"element_type":"minecraft:single_pool_element","location":"x:a","processors":"minecraft:empty","projection":"rigid"}}]}"#),
        ("data/x/worldgen/template_pool/bad.json", br#"{"fallback":"minecraft:empty","elements":[{"weight":1,"element":{"element_type":"x:custom_element","projection":"rigid"}}]}"#),
    ]);
    let r = run(&p);
    assert!(has(&r, "FAIL", "rt_pool_element", "x:custom_element"));
    let rej = &r.findings.items[&("template_pool-rejected".to_string(), "pools rejected by register_dynamic_template_pool_json".to_string())];
    assert_eq!(rej.examples, ["data/x/worldgen/template_pool/bad.json"], "only the bad pool is rejected");
}

#[test]
fn processor_list_with_custom_predicate_is_rejected() {
    let p = pack("proc", &[(
        "data/x/worldgen/processor_list/p.json",
        br#"{"processors":[{"processor_type":"minecraft:rule","rules":[{"input_predicate":{"predicate_type":"minecraft:always_true"},"location_predicate":{"predicate_type":"minecraft:always_true"},"position_predicate":{"predicate_type":"x:air_above"},"output_state":{"Name":"minecraft:air"}}]}]}"#,
    )]);
    let r = run(&p);
    assert!(has(&r, "FAIL", "rt_pos_rule_test", "x:air_above"));
    assert!(r.findings.has("processor_list-rejected", "lists rejected by register_dynamic_processor_list"));
}

#[test]
fn loot_table_semantics_follow_pumpkin_parser() {
    let p = pack("loot", &[
        // unsupported function: dropped, table survives; constant provider object is accepted
        ("data/x/loot_table/a.json", br#"{"pools":[{"rolls":{"type":"minecraft:constant","value":2},"entries":[{"type":"minecraft:item","name":"minecraft:stone","functions":[{"function":"minecraft:set_name","name":"n"}]}]}]}"#),
        // unsupported entry: whole table dropped
        ("data/x/loot_table/b.json", br#"{"pools":[{"rolls":1,"entries":[{"type":"minecraft:made_up"}]}]}"#),
    ]);
    let r = run(&p);
    assert!(has(&r, "WARN", "loot_function", "minecraft:set_name"));
    assert!(!r.findings.items.keys().any(|(c, _)| c == "loot_number_provider"));
    let dropped = &r.findings.items[&("loot_table-dropped".to_string(), "tables that fail to parse (vanilla/static fallback used)".to_string())];
    assert_eq!(dropped.examples, ["data/x/loot_table/b.json"]);
}

#[test]
fn function_entrypoints_mirror_pumpkin_tag_handling() {
    let p = pack("fn", &[
        ("data/minecraft/tags/function/load.json", br##"{"values":["#x:inner",{"id":"x:opt","required":false}]}"##),
        ("data/x/tags/function/inner.json", br#"{"values":["x:main"]}"#),
        ("data/x/function/main.mcfunction", b"say hi\nexecute as @a run function x:child\n$say $(m)\n"),
        ("data/x/function/child.mcfunction", b"schedule function x:later 1t\n"),
        ("data/x/function/later.mcfunction", b"notacommand foo\n"),
        ("data/x/function/opt.mcfunction", b"say opt\n"),
    ]);
    let r = run(&p);
    assert_eq!((r.reach.functions, r.reach.vanilla, r.reach.pumpkin), (4, 4, 0));
    assert!(r.findings.has("function-entrypoint", "pack functions unreachable in Pumpkin"));
    assert!(has(&r, "FAIL", "function-tag-nested-tag", "#tag entry inside a function tag never runs"));
    assert!(has(&r, "FAIL", "function-tag-object-entry", "{id, required} entry dropped"));
    assert!(has(&r, "FAIL", "function-syntax", "macro line '$'"));
    assert!(has(&r, "FAIL", "command", "notacommand"));
    assert!(!r.findings.has("command", "schedule"));
}

#[test]
fn reachability_helpers() {
    assert_eq!(function_calls("execute if x run function a:b\nfunction #c:d\nfunction $(id)\n"), ["a:b", "#c:d"]);
    let mut fns = BTreeMap::new();
    fns.insert("a:f".to_string(), vec![]);
    let mut tags = BTreeMap::new();
    tags.insert("minecraft:tick".to_string(), vec![("a:f".to_string(), false)]);
    assert_eq!(reachable(&fns, &tags, false).0.len(), 1, "plain string entries run in Pumpkin");
}

#[test]
fn json_and_nbt_failures_are_reported() {
    let p = pack("parse", &[
        ("data/x/recipe/trailing.json", b"{\"type\":\"minecraft:crafting_shaped\",}"),
        ("data/x/recipe/odd.json", br#"{"type":"x:machine"}"#),
        ("data/x/structure/bad.nbt", b"not gzip"),
    ]);
    let r = run(&p);
    assert!(r.findings.has("json-parse", "invalid JSON (serde_json rejects; loader skips silently)"));
    assert!(has(&r, "FAIL", "recipe", "x:machine"));
    assert!(has(&r, "FAIL", "structure-nbt", "not gzip (read_gzip_compound_tag fails)"));
}

#[test]
fn worldgen_registry_side_effects() {
    let p = pack("wg", &[
        ("data/x/worldgen/biome/b.json", br#"{"features":[["x:f"]],"carvers":["x:c"]}"#),
        ("data/x/worldgen/structure/s.json", br#"{"type":"x:custom"}"#),
        ("data/c/worldgen/biome_colors.json", b"{}"),
    ]);
    let r = run(&p);
    assert!(has(&r, "FAIL", "biome-id", "custom biome ids"));
    assert!(has(&r, "FAIL", "structure_type", "x:custom"));
    assert!(has(&r, "FAIL", "structure-id", "custom structure ids"));
    assert!(has(&r, "WARN", "client-registry-sync", "minecraft:worldgen/structure"));
    assert!(!r.findings.has("client-registry-sync", "minecraft:worldgen/biome"), "biome is a vanilla synced registry");
    assert!(!r.findings.has("client-registry-sync", "c:worldgen"), "files directly under worldgen/ are not loaded");
}

/// The runtime smoke pack must stay statically clean so that any runtime failure
/// observed with it points at Pumpkin, not at the pack.
#[test]
fn phase2_smoke_pack_is_statically_clean() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/datapacks/phase2-smoke");
    let r = audit_pack(&Pack::open(&p), cap());
    let bad: Vec<_> = r.findings.items.values().filter(|f| f.severity != "INFO").map(|f| (&f.category, &f.item)).collect();
    assert!(bad.is_empty(), "unexpected findings: {bad:?}");
    assert!(r.findings.has("advancement-not-consumed", "datapack advancements are never granted"), "P2-A1 is the known gap");
    // load + tick are entry points in both; probe/chain/nested are run by hand from the console.
    assert_eq!((r.reach.functions, r.reach.vanilla, r.reach.pumpkin), (6, 2, 2));
}

#[test]
fn zip_packs_are_flagged_not_loaded() {
    let dir = std::env::temp_dir().join(format!("datapack-audit-{}-zip", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let z = dir.join("pack.zip");
    fs::write(&z, b"PK\x05\x06").unwrap();
    let r = audit_pack(&Pack::open(&z), cap());
    assert!(has(&r, "FAIL", "packaging", "zip datapack"));
    let _ = fs::remove_dir_all(dir);
}
