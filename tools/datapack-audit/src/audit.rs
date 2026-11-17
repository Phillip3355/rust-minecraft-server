//! Per-file checks. Each check simulates the behaviour of a named Pumpkin code path.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::caps::{ns, Capabilities, Set};
use crate::json::{self, Value};

pub const FAIL: &str = "FAIL";
pub const WARN: &str = "WARN";
pub const INFO: &str = "INFO";

#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: &'static str,
    pub category: String,
    pub item: String,
    pub count: usize,
    pub examples: Vec<String>,
    pub detail: String,
}

#[derive(Default, Debug)]
pub struct Findings {
    pub items: BTreeMap<(String, String), Finding>,
}

impl Findings {
    pub fn add(&mut self, sev: &'static str, cat: &str, item: &str, at: &str, detail: &str) {
        let e = self.items.entry((cat.to_string(), item.to_string())).or_insert_with(|| Finding {
            severity: sev,
            category: cat.to_string(),
            item: item.to_string(),
            count: 0,
            examples: Vec::new(),
            detail: detail.to_string(),
        });
        e.count += 1;
        e.examples.push(at.to_string());
    }
    pub fn has(&self, cat: &str, item: &str) -> bool {
        self.items.contains_key(&(cat.to_string(), item.to_string()))
    }
}

// ------------------------------------------------------------------ pack access

pub struct Pack {
    pub root: PathBuf,
    pub name: String,
    pub is_zip: bool,
}

impl Pack {
    pub fn open(p: &Path) -> Pack {
        let is_zip = p.is_file();
        let name = if is_zip { p.file_stem() } else { p.file_name() }.map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        Pack { root: p.to_path_buf(), name, is_zip }
    }

    pub fn files(&self) -> Vec<String> {
        fn rec(base: &Path, dir: &Path, out: &mut Vec<String>) {
            let Ok(rd) = fs::read_dir(dir) else { return };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    rec(base, &p, out);
                } else if let Ok(r) = p.strip_prefix(base) {
                    out.push(r.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let mut v = Vec::new();
        if !self.is_zip {
            rec(&self.root, &self.root, &mut v);
        }
        v.sort();
        v
    }

    pub fn read(&self, rel: &str) -> std::io::Result<Vec<u8>> {
        fs::read(self.root.join(rel))
    }
}

/// `data/<ns>/<registry>/<path>.<ext>` -> (namespace, registry dir, entry path)
pub fn registry_of(rel: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() < 4 || parts[0] != "data" {
        return None;
    }
    let namespace = parts[1].to_string();
    let rest = &parts[2..];
    let (reg, tail) = if rest[0] == "worldgen" && rest.len() >= 3 {
        (format!("worldgen/{}", rest[1]), &rest[2..])
    } else if rest[0] == "tags" && rest.len() >= 3 {
        if rest[1] == "worldgen" && rest.len() >= 4 {
            (format!("tags/worldgen/{}", rest[2]), &rest[3..])
        } else {
            (format!("tags/{}", rest[1]), &rest[2..])
        }
    } else {
        (rest[0].to_string(), &rest[1..])
    };
    let mut entry = tail.join("/");
    if let Some(dot) = entry.rfind('.') {
        if !entry[dot..].contains('/') {
            entry.truncate(dot);
        }
    }
    Some((namespace, reg, entry))
}

fn singular(reg: &str) -> &str {
    // Pumpkin accepts both `recipe` and legacy `recipes` etc. for top-level dirs.
    match reg {
        "recipes" => "recipe",
        "functions" => "function",
        "loot_tables" => "loot_table",
        "advancements" => "advancement",
        "predicates" => "predicate",
        "item_modifiers" => "item_modifier",
        "structures" => "structure",
        "tags/functions" => "tags/function",
        "damage_types" => "damage_type",
        // 26.x renamed these dirs; Pumpkin's DYNAMIC_REGISTRIES accepts both spellings.
        "worldgen/feature" => "worldgen/configured_feature",
        "worldgen/carver" => "worldgen/configured_carver",
        r => r,
    }
}

// ------------------------------------------------------------------ disposition

pub const ACTIVE: &str = "ACTIVE";
pub const PARTIAL: &str = "PARTIAL";
pub const LOADED_UNUSED: &str = "LOADED_UNUSED";
pub const CLIENT_ONLY: &str = "CLIENT_SYNC_ONLY";
pub const IGNORED_WG: &str = "IGNORED_BY_WORLDGEN";
pub const NOT_LOADED: &str = "NOT_LOADED";

pub fn disposition(reg: &str, cap: &Capabilities) -> (&'static str, String) {
    let r = singular(reg);
    let fixed: Option<(&'static str, &str)> = match r {
        "function" => Some((ACTIVE, "function_loader.rs; lines run through CommandDispatcher; #minecraft:load after start/reload, #minecraft:tick every tick (server/mod.rs)")),
        "tags/function" => Some((PARTIAL, "`replace` ignored; {id,required} object entries dropped (function_loader.rs::load_tags_recursive)")),
        "recipe" => Some((PARTIAL, "recipe_loader.rs; unsupported types return None silently")),
        "loot_table" => Some((PARTIAL, "World::get_loot_table prefers datapack tables; unsupported functions dropped, unsupported conditions => Never, unsupported entry/provider => whole table dropped silently (pumpkin-util loot_table/parser.rs)")),
        "predicate" => Some((PARTIAL, "only resolved while parsing/evaluating loot tables")),
        "advancement" => Some((LOADED_UNUSED, "advancement_loader.rs stores definitions; player tracker and /advancement use static pumpkin_data advancements")),
        "item_modifier" | "slot_source" => Some((LOADED_UNUSED, "no caller of DatapackManager getter")),
        "structure" => Some((ACTIVE, ".nbt registered in the template cache (structure_loader.rs); used by /place template and jigsaw")),
        "worldgen/template_pool" => Some((PARTIAL, "runtime-registered; overrides vanilla pool ids; custom pools only reachable from compiled structures")),
        "worldgen/processor_list" => Some((PARTIAL, "runtime-registered; overrides vanilla ids; unknown processor_type rejects whole list (error! logged)")),
        "worldgen/biome" => Some((CLIENT_ONLY, "merged into synced registry sent to clients; server biomes are compiled (pumpkin-data biome.rs)")),
        "dimension_type" => Some((CLIENT_ONLY, "sent to clients; server heights are compiled (pumpkin-data dimension.rs)")),
        "damage_type" | "trade_set" | "villager_trade" | "test_instance" => Some((ACTIVE, "dedicated loader")),
        _ => None,
    };
    if let Some((s, n)) = fixed {
        return (s, n.to_string());
    }
    if r == "worldgen" {
        return (NOT_LOADED, "files directly under worldgen/ are not visited (dynamic_registry_loader.rs only descends into subdirs)".into());
    }
    if r.starts_with("tags/") {
        return (LOADED_UNUSED, "tag_loader.rs map has no gameplay caller; clients receive only static tags (login/known_packs.rs); nested #refs unresolved".into());
    }
    if r.starts_with("worldgen/") || r == "dimension" {
        return (IGNORED_WG, "worldgen is generated at compile time from vanilla JSON (tools/pumpkin-codegen); runtime only stores raw NBT in a dynamic registry".into());
    }
    if let Some(id) = cap.dynamic_registry_dirs.get(r) {
        if cap.synced_registries.contains(id) {
            return (CLIENT_ONLY, format!("dynamic registry {id}: merged and synced to client"));
        }
        return (LOADED_UNUSED, format!("dynamic registry {id}: not vanilla-synced but still appended to client registry data"));
    }
    if cap.excluded_registry_dirs.contains(r) {
        return (NOT_LOADED, "excluded dir".into());
    }
    (LOADED_UNUSED, "auto-discovered custom registry folder (dynamic_registry_loader.rs); appended to client registry data".into())
}

/// Registry id the dynamic registry loader files a dir under (None = not a dynamic registry).
pub fn dynamic_registry_id(namespace: &str, reg: &str, cap: &Capabilities) -> Option<String> {
    if let Some(id) = cap.dynamic_registry_dirs.get(reg) {
        return Some(id.clone());
    }
    let top = reg.split('/').next().unwrap_or(reg);
    // Files directly under `worldgen/` are skipped: auto-discovery only descends into its subdirs.
    if reg == "worldgen" || top == "tags" || cap.excluded_registry_dirs.contains(top) || cap.excluded_registry_dirs.contains(reg) {
        return None;
    }
    Some(format!("{namespace}:{reg}"))
}

// ------------------------------------------------------------------ function reachability

/// Function ids referenced by `function <id>` (covers `run function`, `schedule function`).
pub fn function_calls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        let toks: Vec<&str> = s.trim_start_matches(['/', '$']).split_whitespace().collect();
        for w in toks.windows(2) {
            if w[0] == "function" {
                let t = w[1];
                if t.is_empty() || t.contains("$(") || t.starts_with('{') {
                    continue; // macro-built id: not statically resolvable
                }
                out.push(if let Some(tag) = t.strip_prefix('#') { format!("#{}", ns(tag)) } else { ns(t) });
            }
        }
    }
    out
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Reachability {
    pub functions: usize,
    pub vanilla: usize,
    pub pumpkin: usize,
    pub vanilla_entry: Vec<String>,
    pub pumpkin_entry: Vec<String>,
}

/// Functions reachable from `#minecraft:load` / `#minecraft:tick`.
/// `vanilla`: nested `#tags` and `{id}` objects in tags expand. Pumpkin: neither does
/// (function_loader.rs keeps string values only; visit_function_lines does not recurse).
pub fn reachable(functions: &BTreeMap<String, Vec<String>>, tags: &BTreeMap<String, Vec<(String, bool)>>, vanilla: bool) -> (BTreeSet<String>, Vec<String>) {
    let expand = |tag: &str| -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![tag.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(t) = stack.pop() {
            if !seen.insert(t.clone()) {
                continue;
            }
            for (id, is_obj) in tags.get(&t).map(Vec::as_slice).unwrap_or(&[]) {
                if !vanilla && *is_obj {
                    continue;
                }
                match id.strip_prefix('#') {
                    Some(inner) if vanilla => stack.push(inner.to_string()),
                    Some(_) => {}
                    None => out.push(id.clone()),
                }
            }
        }
        out
    };
    let mut entry = Vec::new();
    for t in ["minecraft:load", "minecraft:tick"] {
        entry.extend(expand(t));
    }
    let mut seen = BTreeSet::new();
    let mut stack = entry.clone();
    while let Some(fid) = stack.pop() {
        if let Some(tag) = fid.strip_prefix('#') {
            stack.extend(expand(tag));
            continue;
        }
        if !functions.contains_key(&fid) || !seen.insert(fid.clone()) {
            continue;
        }
        stack.extend(functions[&fid].iter().cloned());
    }
    entry.sort();
    entry.dedup();
    (seen, entry)
}

// ------------------------------------------------------------------ audit

pub struct PackResult {
    pub reach: Reachability,
    pub name: String,
    pub findings: Findings,
    /// (namespace, registry, ext) -> files
    pub inventory: BTreeMap<(String, String, String), usize>,
    pub disposition: BTreeMap<String, (&'static str, String)>,
    pub command_usage: BTreeMap<String, usize>,
    /// registry dir -> ids this pack defines
    pub own_ids: BTreeMap<String, Set>,
}

pub fn audit_pack(pack: &Pack, cap: &Capabilities) -> PackResult {
    let mut r = PackResult {
        reach: Reachability::default(),
        name: pack.name.clone(),
        findings: Findings::default(),
        inventory: BTreeMap::new(),
        disposition: BTreeMap::new(),
        command_usage: BTreeMap::new(),
        own_ids: BTreeMap::new(),
    };
    let f = &mut r.findings;
    if pack.is_zip {
        f.add(FAIL, "packaging", "zip datapack", &pack.name, "scan_datapacks_dir() skips non-directories: ZIPs are listed by /datapack but never loaded");
        return r;
    }
    check_mcmeta(f, pack);

    let files = pack.files();
    for rel in &files {
        if let Some((n, reg, entry)) = registry_of(rel) {
            r.own_ids.entry(singular(&reg).to_string()).or_default().insert(format!("{n}:{entry}"));
        }
    }
    let mut new_client_registries: BTreeSet<String> = BTreeSet::new();
    let mut functions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut ftags: BTreeMap<String, Vec<(String, bool)>> = BTreeMap::new();
    for rel in &files {
        let Some((namespace, reg, entry)) = registry_of(rel) else { continue };
        let ext = rel.rfind('.').map(|i| rel[i..].to_string()).unwrap_or_default();
        *r.inventory.entry((namespace.clone(), reg.clone(), ext.clone())).or_default() += 1;
        r.disposition.entry(reg.clone()).or_insert_with(|| disposition(&reg, cap));
        let rid = format!("{namespace}:{entry}");
        let Ok(raw) = pack.read(rel) else {
            f.add(FAIL, "io", "unreadable file", rel, "");
            continue;
        };
        match ext.as_str() {
            ".json" => {
                let Ok(text) = String::from_utf8(raw) else {
                    f.add(FAIL, "json-parse", "not UTF-8", rel, "fs::read_to_string fails; file silently skipped");
                    continue;
                };
                let doc = match json::parse(&text) {
                    Ok(d) => d,
                    Err(e) => {
                        f.add(FAIL, "json-parse", "invalid JSON (serde_json rejects; loader skips silently)", rel, &e.to_string());
                        continue;
                    }
                };
                if let Some(id) = dynamic_registry_id(&namespace, singular(&reg), cap) {
                    if !cap.synced_registries.contains(&id) {
                        new_client_registries.insert(id);
                    }
                }
                if singular(&reg) == "tags/function" {
                    let e = ftags.entry(rid.clone()).or_default();
                    for v in doc.get("values").map(Value::many).unwrap_or(&[]) {
                        if let Some(s) = v.as_str() {
                            e.push((s.to_string(), false));
                        } else if let Some(s) = v.str_of("id") {
                            e.push((s.to_string(), true));
                        }
                    }
                    for (id, _) in e.iter_mut() {
                        *id = id.strip_prefix('#').map_or_else(|| ns(id), |t| format!("#{}", ns(t)));
                    }
                }
                check_json(f, cap, &r.own_ids, singular(&reg), rel, &rid, &doc);
            }
            ".mcfunction" => match String::from_utf8(raw) {
                Ok(t) => {
                    if singular(&reg) == "function" {
                        functions.insert(rid.clone(), function_calls(&t));
                    }
                    check_function(f, cap, rel, &t, &mut r.command_usage)
                }
                Err(_) => f.add(FAIL, "function", "not UTF-8", rel, "fs::read_to_string fails; function silently skipped"),
            },
            ".nbt" => check_nbt(f, rel, &raw),
            _ => {}
        }
    }
    let (van, van_entry) = reachable(&functions, &ftags, true);
    let (pum, pum_entry) = reachable(&functions, &ftags, false);
    r.reach = Reachability { functions: functions.len(), vanilla: van.len(), pumpkin: pum.len(), vanilla_entry: van_entry, pumpkin_entry: pum_entry };
    if r.reach.pumpkin < r.reach.vanilla {
        let d = format!(
            "{} of {} functions run from #minecraft:load/#minecraft:tick in vanilla, {} in Pumpkin (entry points vanilla={:?}, pumpkin={:?})",
            r.reach.vanilla, r.reach.functions, r.reach.pumpkin, r.reach.vanilla_entry, r.reach.pumpkin_entry
        );
        f.add(FAIL, "function-entrypoint", "pack functions unreachable in Pumpkin", &pack.name, &d);
    }
    for id in new_client_registries {
        f.add(WARN, "client-registry-sync", &id, &pack.name, "not in Pumpkin's base synced set: DynamicRegistryManager::apply_custom_registries appends it and handle_known_packs sends it to every Java client (client reaction unverified)");
    }
    r
}

fn check_mcmeta(f: &mut Findings, pack: &Pack) {
    let meta = match pack.read("pack.mcmeta").ok().and_then(|b| String::from_utf8(b).ok()).map(|t| json::parse(&t)) {
        Some(Ok(v)) => v,
        _ => {
            f.add(WARN, "pack.mcmeta", "missing or unparsable", "pack.mcmeta", "read_pack_mcmeta() falls back to empty description and format 61");
            return;
        }
    };
    let p = meta.get("pack");
    let fmt = p.and_then(|p| p.get("pack_format").or_else(|| p.get("max_format")).or_else(|| p.get("min_format")));
    if !fmt.is_some_and(|v| v.as_i64().is_some()) {
        f.add(WARN, "pack.mcmeta", "pack format is not a plain integer", "pack.mcmeta", "read_pack_mcmeta() uses as_u64() and falls back to 61: /datapack list reports a wrong format (cosmetic)");
    }
    if meta.get("overlays").and_then(|o| o.get("entries")).and_then(Value::as_arr).is_some_and(|a| !a.is_empty()) {
        f.add(FAIL, "pack.mcmeta", "overlays used", "pack.mcmeta", "Pumpkin has no overlay support");
    }
    if p.and_then(|p| p.get("description")).is_some_and(|d| !matches!(d, Value::Str(_))) {
        f.add(INFO, "pack.mcmeta", "text-component description", "pack.mcmeta", "stored as raw JSON string by read_pack_mcmeta()");
    }
}

fn check_nbt(f: &mut Findings, rel: &str, raw: &[u8]) {
    match world_verify::compress::gzip_decompress(raw, 64 << 20) {
        Ok(d) if d.first() == Some(&0x0A) => {}
        Ok(_) => f.add(FAIL, "structure-nbt", "root tag is not a compound", rel, ""),
        Err(e) => f.add(FAIL, "structure-nbt", "not gzip (read_gzip_compound_tag fails)", rel, &e.to_string()),
    }
}

pub fn check_function(f: &mut Findings, cap: &Capabilities, rel: &str, text: &str, usage: &mut BTreeMap<String, usize>) {
    for (i, line) in text.lines().enumerate() {
        let at = format!("{rel}:{}", i + 1);
        let s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s.ends_with('\\') {
            f.add(FAIL, "function-syntax", "line continuation '\\'", &at, "function_loader.rs splits on lines; continued commands break");
        }
        if s.starts_with('$') {
            f.add(FAIL, "function-syntax", "macro line '$'", &at, "no macro support; line is dispatched verbatim and fails");
            continue;
        }
        let s = s.strip_prefix('/').unwrap_or(s);
        let mut cmds = vec![s.split_whitespace().next().unwrap_or("").to_string()];
        let toks: Vec<&str> = s.split_whitespace().collect();
        for w in toks.windows(2) {
            if w[0] == "run" {
                cmds.push(w[1].to_string());
            }
        }
        for c in cmds {
            *usage.entry(c.clone()).or_default() += 1;
            if !cap.commands.contains(&c) {
                f.add(FAIL, "command", &c, &at, "not registered in command/commands/mod.rs::default_dispatcher");
            }
        }
    }
}

// ------------------------------------------------------------------ JSON checks

fn walk<'a>(v: &'a Value, path: &mut Vec<String>, f: &mut dyn FnMut(&'a Value, &[String])) {
    f(v, path);
    match v {
        Value::Obj(m) => {
            for (k, x) in m {
                path.push(k.clone());
                walk(x, path, f);
                path.pop();
            }
        }
        Value::Arr(a) => {
            for x in a {
                walk(x, path, f);
            }
        }
        _ => {}
    }
}

fn types_in(v: &Value, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(v, &mut Vec::new(), &mut |x, _| {
        if let Some(t) = x.str_of(key) {
            out.push(ns(t));
        }
    });
    out
}

fn need(f: &mut Findings, cap: &Capabilities, cat: &str, t: &str, at: &str, sev: &'static str, detail: &str) -> bool {
    if cap.has(cat, t) {
        return true;
    }
    f.add(sev, cat, t, at, detail);
    false
}

const VANILLA_NOISE_SETTINGS: &[&str] = &["overworld", "amplified", "large_biomes", "nether", "end", "caves", "floating_islands"];
const ROUTERS: &[&str] = &["overworld", "amplified", "large_biomes", "nether", "end"];
const FEATURE_NESTED: &[&str] = &["feature", "block_state_provider", "trunk_placer", "foliage_placer", "tree_decorator", "root_placer", "feature_size", "placement_modifier", "block_predicate", "int_provider", "height_provider", "float_provider", "rule_test"];

pub fn check_json(f: &mut Findings, cap: &Capabilities, own: &BTreeMap<String, Set>, reg: &str, rel: &str, rid: &str, doc: &Value) {
    let custom_ns = !rid.starts_with("minecraft:");
    match reg {
        "worldgen/density_function" => {
            for t in types_in(doc, "type") {
                need(f, cap, "density_function", &t, rel, FAIL, "noise_router.rs::parse_vanilla_df panics on unknown type");
            }
            check_df_refs(f, cap, rel, doc);
            if custom_ns {
                f.add(FAIL, "density_function-id", "custom density_function ids", rel, "no runtime DF registry: routers are compiled constants (noise_router.rs codegen)");
            } else {
                f.add(INFO, "density_function-override", rid, rel, "vanilla DF override has no effect (compiled router)");
            }
        }
        "worldgen/noise_settings" => {
            let router = doc.get("noise_router").cloned().unwrap_or(Value::Null);
            for t in types_in(&router, "type") {
                need(f, cap, "density_function", &t, rel, FAIL, "noise_router.rs::parse_vanilla_df panics on unknown type");
            }
            check_df_refs(f, cap, rel, &router);
            if let Some(sr) = doc.get("surface_rule") {
                for t in types_in(sr, "type") {
                    if !cap.has("surface_rule", &t) && !cap.has("surface_condition", &t) {
                        f.add(FAIL, "surface_rule", &t, rel, "material_rule.rs serde enums (DirectMaterialRule/DirectMaterialCondition)");
                    }
                }
            }
            let path = rid.split_once(':').map_or(rid, |x| x.1);
            if custom_ns || !VANILLA_NOISE_SETTINGS.contains(&path) {
                f.add(FAIL, "noise_settings-id", "custom noise_settings ids", rel, "BaseNoiseRouters::from_name only knows overworld/amplified/large_biomes/nether/end");
            } else {
                let sev = if ROUTERS.contains(&path) { INFO } else { WARN };
                f.add(sev, "noise_settings-override", rid, rel, "override of vanilla noise_settings has no runtime effect (router, surface rule, sea level, height are compiled)");
            }
        }
        "worldgen/noise" => {
            if !cap.vanilla("worldgen/noise", &rid.replace('/', "_")) {
                f.add(FAIL, "noise-id", "custom noise ids", rel, "DoublePerlinNoiseParameters::id_to_parameters is a closed compile-time list");
            } else {
                f.add(INFO, "noise-override", rid, rel, "vanilla noise parameter override has no effect (compiled)");
            }
        }
        "worldgen/configured_feature" => {
            let top = ns(doc.str_of("type").unwrap_or(""));
            need(f, cap, "feature", &top, rel, FAIL, "configured_feature.rs codegen emits compile_error! for unknown type");
            check_feature_nested(f, cap, own, rel, doc);
            if !cap.vanilla("worldgen/configured_feature", rid) {
                f.add(FAIL, "configured_feature-id", "custom configured_feature ids", rel, "ConfiguredFeature is a closed generated enum");
            } else {
                f.add(INFO, "configured_feature-override", rid, rel, "vanilla override has no effect (compiled)");
            }
        }
        "worldgen/placed_feature" => {
            match doc.get("feature") {
                Some(Value::Str(s)) => check_ref(f, cap, own, "worldgen/configured_feature", s, rel),
                Some(v @ Value::Obj(_)) => {
                    need(f, cap, "feature", &ns(v.str_of("type").unwrap_or("")), rel, FAIL, "inline configured feature");
                }
                _ => {}
            }
            for pm in doc.get("placement").map(Value::many).unwrap_or(&[]) {
                if let Some(t) = pm.str_of("type") {
                    need(f, cap, "placement_modifier", &ns(t), rel, FAIL, "placed_feature.rs codegen emits compile_error! for unknown modifier");
                }
            }
            check_feature_nested(f, cap, own, rel, doc);
            if !cap.vanilla("worldgen/placed_feature", rid) {
                f.add(FAIL, "placed_feature-id", "custom placed_feature ids", rel, "PlacedFeature is a closed generated enum");
            } else {
                f.add(INFO, "placed_feature-override", rid, rel, "vanilla override has no effect (compiled)");
            }
        }
        "worldgen/configured_carver" => {
            let t = ns(doc.str_of("type").unwrap_or(""));
            need(f, cap, "carver", &t, rel, WARN, "carver.rs codegen falls back to the default cave config (silently wrong)");
            if custom_ns {
                f.add(FAIL, "configured_carver-id", "custom configured_carver ids", rel, "carvers are compiled into Biome.carvers");
            } else {
                f.add(INFO, "configured_carver-override", rid, rel, "vanilla override has no effect (compiled)");
            }
        }
        "worldgen/structure" => {
            need(f, cap, "structure_type", &ns(doc.str_of("type").unwrap_or("")), rel, FAIL, "structure_type_to_token => StructureType::Unknown");
            if !cap.vanilla("worldgen/structure", rid) {
                f.add(FAIL, "structure-id", "custom structure ids", rel, "structures.rs::structure_key_to_token panics; StructureKeys is a closed enum");
            } else {
                f.add(INFO, "structure-override", rid, rel, "vanilla override has no effect (compiled)");
            }
        }
        "worldgen/structure_set" => {
            if let Some(pl) = doc.get("placement") {
                need(f, cap, "structure_placement", &ns(pl.str_of("type").unwrap_or("")), rel, FAIL, "StructurePlacementTypeStruct serde enum");
            }
            for s in doc.get("structures").map(Value::many).unwrap_or(&[]) {
                if let Some(id) = s.str_of("structure") {
                    if !cap.vanilla("worldgen/structure", &ns(id)) {
                        f.add(FAIL, "structure_set-ref", "references non-compiled structure", rel, &ns(id));
                    }
                }
            }
            if custom_ns {
                f.add(FAIL, "structure_set-id", "custom structure_set ids", rel, "structure sets are compiled (structures.rs codegen)");
            } else {
                f.add(INFO, "structure_set-override", rid, rel, "vanilla override has no effect (compiled)");
            }
        }
        "worldgen/biome" => {
            if !cap.vanilla("worldgen/biome", rid) {
                f.add(FAIL, "biome-id", "custom biome ids", rel, "Biome is a compiled static set (u8 ids); entry only reaches the client");
            } else {
                f.add(INFO, "biome-override", rid, rel, "server keeps compiled biome (features/carvers/spawns); client gets the override");
            }
            for step in doc.get("features").map(Value::many).unwrap_or(&[]) {
                for pf in step.many() {
                    if let Some(s) = pf.as_str() {
                        if !s.starts_with('#') {
                            check_ref(f, cap, own, "worldgen/placed_feature", s, rel);
                        }
                    }
                }
            }
            for c in doc.get("carvers").map(Value::many).unwrap_or(&[]) {
                if let Some(s) = c.as_str() {
                    if !s.starts_with('#') && !ns(s).starts_with("minecraft:") {
                        f.add(FAIL, "worldgen/configured_carver-ref", "custom configured_carver ref", rel, &ns(s));
                    }
                }
            }
        }
        "worldgen/template_pool" => check_template_pool(f, cap, rel, doc),
        "worldgen/processor_list" => check_processor_list(f, cap, rel, doc),
        "worldgen/multi_noise_biome_source_parameter_list" => {
            f.add(FAIL, "biome_source", rid, rel, "multi-noise parameters come from compiled assets/multi_noise_biome_tree.json");
        }
        "worldgen/world_preset" | "worldgen/flat_level_generator_preset" => {
            f.add(FAIL, "world_preset", rid, rel, "world presets are include_str!'d at compile time (pumpkin-world world_info/mod.rs)");
        }
        "dimension" => {
            let g = doc.get("generator");
            let detail = format!(
                "generator={} settings={} biome_source={}: dimension JSON is not used to build the generator",
                g.and_then(|g| g.str_of("type")).unwrap_or("?"),
                g.and_then(|g| g.str_of("settings")).unwrap_or("(inline)"),
                g.and_then(|g| g.get("biome_source")).and_then(|b| b.str_of("type")).unwrap_or("?")
            );
            f.add(FAIL, "dimension-override", rid, rel, &detail);
        }
        "dimension_type" => {
            let d = format!(
                "min_y={} height={} logical_height={}: client only; server keeps compiled heights",
                doc.get("min_y").and_then(Value::as_i64).map_or("?".into(), |v| v.to_string()),
                doc.get("height").and_then(Value::as_i64).map_or("?".into(), |v| v.to_string()),
                doc.get("logical_height").and_then(Value::as_i64).map_or("?".into(), |v| v.to_string())
            );
            f.add(WARN, "dimension_type-override", rid, rel, &d);
        }
        "recipe" => {
            let t = ns(doc.str_of("type").unwrap_or(""));
            if !t.starts_with("minecraft:crafting_special_") {
                need(f, cap, "recipe", &t, rel, FAIL, "parse_recipe() returns None silently");
            }
        }
        "loot_table" => check_loot(f, cap, rel, doc),
        "advancement" => f.add(INFO, "advancement-not-consumed", "datapack advancements are never granted", rel, "see disposition"),
        r if r.starts_with("tags/") => {
            let vals = doc.get("values").map(Value::many).unwrap_or(&[]);
            let nested = vals.iter().any(|v| v.as_str().is_some_and(|s| s.starts_with('#')));
            if r == "tags/function" {
                if nested {
                    f.add(FAIL, "function-tag-nested-tag", "#tag entry inside a function tag never runs", rel, "visit_function_lines looks `#ns:x` up as a function id, finds nothing and skips it");
                }
                if vals.iter().any(Value::is_obj) {
                    f.add(FAIL, "function-tag-object-entry", "{id, required} entry dropped", rel, "function_loader.rs keeps only string values, so the referenced function never runs");
                }
                if doc.get("replace") == Some(&Value::Bool(true)) {
                    f.add(WARN, "function-tag-replace", "replace:true ignored", rel, "entries are appended");
                }
            } else if nested {
                f.add(WARN, "tag-nested-ref", r, rel, "nested #tag kept as a literal; DatapackManager::is_in_tag does not expand it");
            }
        }
        _ => {}
    }
}

fn check_ref(f: &mut Findings, cap: &Capabilities, own: &BTreeMap<String, Set>, registry: &str, r: &str, rel: &str) {
    let id = ns(r);
    if cap.vanilla(registry, &id) {
        return;
    }
    let cat = format!("{registry}-ref");
    if own.get(registry).is_some_and(|s| s.contains(&id)) {
        f.add(FAIL, &cat, "reference to a pack-defined id (not compiled into Pumpkin)", rel, &id);
    } else {
        f.add(FAIL, &cat, "unresolved reference (not compiled, not in this pack)", rel, &id);
    }
}

fn check_df_refs(f: &mut Findings, cap: &Capabilities, rel: &str, doc: &Value) {
    walk(doc, &mut Vec::new(), &mut |v, path| {
        let last = path.last().map(String::as_str).unwrap_or("");
        if let Value::Str(s) = v {
            if s.contains(':') && last != "noise" && last != "type" && !s.starts_with("minecraft:") {
                f.add(FAIL, "density_function-ref", "non-minecraft density function reference", rel, "noise_router.rs::load_df_json strips only `minecraft:` and reads the vanilla dir");
            }
        }
        if let Some(n) = v.str_of("noise") {
            let id = ns(n).replace('/', "_");
            if !cap.vanilla("worldgen/noise", &id) {
                f.add(FAIL, "noise-ref", "non-compiled noise reference", rel, &ns(n));
            }
        }
    });
}

fn check_feature_nested(f: &mut Findings, cap: &Capabilities, own: &BTreeMap<String, Set>, rel: &str, doc: &Value) {
    let union: Set = FEATURE_NESTED.iter().filter_map(|c| cap.types.get(*c)).flatten().cloned().collect();
    let mut refs = Vec::new();
    walk(doc, &mut Vec::new(), &mut |v, path| {
        if path.is_empty() {
            return;
        }
        for key in ["type", "predicate_type"] {
            if let Some(t) = v.str_of(key) {
                let t = ns(t);
                if !union.contains(&t) {
                    f.add(FAIL, "feature-nested-type", &t, rel, &format!("inner `{key}` under /{} unknown to feature codegen", path.join("/")));
                }
            }
        }
        if let Some(s) = v.str_of("feature") {
            // An inline placed feature `{feature, placement}` points at a configured feature;
            // elsewhere (random_selector entries, random_patch, vegetation_patch) it is a placed feature.
            let reg = if v.get("placement").is_some() { "worldgen/configured_feature" } else { "worldgen/placed_feature" };
            refs.push((reg, s.to_string()));
        }
    });
    for (reg, r) in refs {
        check_ref(f, cap, own, reg, &r, rel);
    }
}

fn check_template_pool(f: &mut Findings, cap: &Capabilities, rel: &str, doc: &Value) {
    let mut ok = true;
    for k in ["fallback", "elements"] {
        if doc.get(k).is_none() {
            f.add(FAIL, "template_pool", &format!("missing `{k}`"), rel, "serde error: whole pool rejected (error! logged)");
            ok = false;
        }
    }
    for el in doc.get("elements").map(Value::many).unwrap_or(&[]) {
        if el.get("weight").and_then(Value::as_i64).is_none_or(|w| w < 0) {
            f.add(FAIL, "template_pool", "missing/negative weight", rel, "serde u32: whole pool rejected");
            ok = false;
        }
        if let Some(e) = el.get("element") {
            ok &= check_pool_element(f, cap, rel, e);
        }
    }
    if !ok {
        f.add(FAIL, "template_pool-rejected", "pools rejected by register_dynamic_template_pool_json", rel, "");
    }
}

/// Returns false when serde would reject the element (and therefore the whole pool).
fn check_pool_element(f: &mut Findings, cap: &Capabilities, rel: &str, e: &Value) -> bool {
    let et = e.str_of("element_type").unwrap_or("");
    if !cap.has("rt_pool_element", et) {
        f.add(FAIL, "rt_pool_element", et, rel, "unknown element_type: serde error, whole template_pool rejected");
        return false;
    }
    let mut ok = true;
    if et != "minecraft:empty_pool_element" && !matches!(e.str_of("projection"), Some("rigid" | "terrain_matching")) {
        f.add(FAIL, "rt_pool_element", "missing/unknown projection", rel, "serde error: pool rejected");
        ok = false;
    }
    if et.ends_with("single_pool_element") {
        match e.get("processors") {
            None => {
                f.add(FAIL, "rt_pool_element", "missing processors", rel, "serde error: pool rejected");
                ok = false;
            }
            Some(p @ Value::Obj(_)) => {
                if p.get("processors").and_then(Value::as_arr).is_some_and(|a| !a.is_empty()) {
                    f.add(WARN, "rt_pool_element", "inline processor list ignored", rel, "jigsaw.rs: RawProcessorList::Inline => ProcessorListRef::Empty");
                }
            }
            _ => {}
        }
    }
    if et == "minecraft:feature_pool_element" {
        let feat = ns(e.str_of("feature").unwrap_or(""));
        if !cap.vanilla("worldgen/placed_feature", &feat) {
            f.add(WARN, "rt_pool_element", "feature_pool_element with non-compiled placed_feature dropped", rel, &feat);
        }
    }
    if et == "minecraft:list_pool_element" {
        for sub in e.get("elements").map(Value::many).unwrap_or(&[]) {
            ok &= check_pool_element(f, cap, rel, sub);
        }
    }
    ok
}

fn check_processor_list(f: &mut Findings, cap: &Capabilities, rel: &str, doc: &Value) {
    let procs = doc.get("processors").unwrap_or(doc).many();
    let mut ok = true;
    for p in procs {
        let pt = ns(p.str_of("processor_type").unwrap_or(""));
        if !cap.has("rt_processor", &pt) {
            f.add(FAIL, "rt_processor", &pt, rel, "unknown processor_type: serde error, whole processor_list rejected (error! logged)");
            ok = false;
            continue;
        }
        for rule in p.get("rules").map(Value::many).unwrap_or(&[]) {
            for (key, cat) in [("input_predicate", "rt_rule_test"), ("location_predicate", "rt_rule_test"), ("position_predicate", "rt_pos_rule_test")] {
                if let Some(t) = rule.get(key).and_then(|x| x.str_of("predicate_type")) {
                    if !need(f, cap, cat, &ns(t), rel, FAIL, "unknown predicate_type: processor_list rejected") {
                        ok = false;
                    }
                }
            }
            if let Some(t) = rule.get("block_entity_modifier").and_then(|x| x.str_of("type")) {
                if !need(f, cap, "rt_block_entity_modifier", &ns(t), rel, FAIL, "unknown modifier: processor_list rejected") {
                    ok = false;
                }
            }
        }
    }
    if !ok {
        f.add(FAIL, "processor_list-rejected", "lists rejected by register_dynamic_processor_list", rel, "");
    }
}

fn check_loot(f: &mut Findings, cap: &Capabilities, rel: &str, doc: &Value) {
    let mut ok = true;
    fn provider_ok(cap: &Capabilities, v: &Value) -> bool {
        // parser.rs::scalar accepts a bare number or any object with a numeric `value`.
        if v.is_num() || v.get("value").is_some_and(Value::is_num) {
            return true;
        }
        let t = v.str_of("type").map_or("minecraft:uniform".to_string(), ns);
        v.is_obj() && cap.has("loot_number_provider", &t)
    }
    fn conds(f: &mut Findings, cap: &Capabilities, rel: &str, v: &Value) {
        if let Some(c) = v.get("conditions").or_else(|| v.get("condition")) {
            for x in c.many() {
                if let Some(t) = x.str_of("condition").or_else(|| x.str_of("type")) {
                    need(f, cap, "loot_condition", &ns(t), rel, WARN, "unsupported condition => Condition::Never (entry never drops)");
                }
            }
        }
    }
    fn funcs(f: &mut Findings, cap: &Capabilities, rel: &str, v: &Value) {
        if let Some(fs) = v.get("functions").or_else(|| v.get("modifier")) {
            for x in fs.many() {
                if let Some(t) = x.str_of("function").or_else(|| x.str_of("type")) {
                    need(f, cap, "loot_function", &ns(t), rel, WARN, "unsupported function silently dropped (filter_map)");
                }
            }
        }
    }
    fn entry(f: &mut Findings, cap: &Capabilities, rel: &str, e: &Value, ok: &mut bool) {
        let t = ns(e.str_of("type").unwrap_or(""));
        if !need(f, cap, "loot_entry", &t, rel, FAIL, "unsupported entry => Parser::entry returns None => whole table dropped silently") {
            *ok = false;
        }
        for c in e.get("children").map(Value::many).unwrap_or(&[]) {
            entry(f, cap, rel, c, ok);
        }
        conds(f, cap, rel, e);
        funcs(f, cap, rel, e);
    }
    for pool in doc.get("pools").map(Value::many).unwrap_or(&[]) {
        if pool.get("entries").and_then(Value::as_arr).is_none() {
            f.add(FAIL, "loot_pool", "missing entries", rel, "pool parse fails => table dropped");
            ok = false;
        }
        for k in ["rolls", "bonus_rolls"] {
            if let Some(v) = pool.get(k) {
                if !provider_ok(cap, v) {
                    f.add(FAIL, "loot_number_provider", v.str_of("type").unwrap_or("?"), rel, "unsupported rolls provider => table dropped");
                    ok = false;
                }
            }
        }
        for e in pool.get("entries").map(Value::many).unwrap_or(&[]) {
            entry(f, cap, rel, e, &mut ok);
        }
        conds(f, cap, rel, pool);
        funcs(f, cap, rel, pool);
    }
    funcs(f, cap, rel, doc);
    if !ok {
        f.add(FAIL, "loot_table-dropped", "tables that fail to parse (vanilla/static fallback used)", rel, "");
    }
}
