//! JSON and Markdown output.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::audit::{Finding, PackResult};
use crate::caps::Capabilities;
use crate::json::quote;

fn sev_rank(s: &str) -> u8 {
    match s {
        "FAIL" => 0,
        "WARN" => 1,
        _ => 2,
    }
}

pub fn sorted_findings(r: &PackResult) -> Vec<&Finding> {
    let mut v: Vec<&Finding> = r.findings.items.values().collect();
    v.sort_by(|a, b| (sev_rank(a.severity), &a.category, &a.item).cmp(&(sev_rank(b.severity), &b.category, &b.item)));
    v
}

pub fn totals(r: &PackResult) -> BTreeMap<&'static str, (usize, usize)> {
    let mut t: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
    for f in r.findings.items.values() {
        let e = t.entry(f.severity).or_default();
        e.0 += 1;
        e.1 += f.count;
    }
    t
}

fn arr(items: impl IntoIterator<Item = String>) -> String {
    let v: Vec<String> = items.into_iter().map(|s| quote(&s)).collect();
    format!("[{}]", v.join(","))
}

pub fn to_json(cap: &Capabilities, results: &[PackResult], pumpkin_rev: &str) -> String {
    let mut o = String::from("{\n");
    let _ = writeln!(o, " \"tool\": \"datapack-audit {}\",", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(o, " \"pumpkin_rev\": {},", quote(pumpkin_rev));
    o.push_str(" \"capabilities\": {\n");
    let caps: Vec<String> = cap.types.iter().map(|(k, v)| format!("  {}: {{\"sources\": {}, \"types\": {}}}", quote(k), arr(cap.sources.get(k).cloned().unwrap_or_default()), arr(v.iter().cloned()))).collect();
    o.push_str(&caps.join(",\n"));
    o.push_str("\n },\n");
    let _ = writeln!(o, " \"commands\": {},", arr(cap.commands.iter().cloned()));
    let vid: Vec<String> = cap.vanilla_ids.iter().map(|(k, v)| format!("{}: {}", quote(k), v.len())).collect();
    let _ = writeln!(o, " \"compiled_vanilla_id_counts\": {{{}}},", vid.join(", "));
    o.push_str(" \"packs\": [\n");
    let packs: Vec<String> = results
        .iter()
        .map(|r| {
            let mut p = String::new();
            let _ = writeln!(p, "  {{\"name\": {},", quote(&r.name));
            let inv: Vec<String> = r.inventory.iter().map(|((n, g, e), c)| format!("   {{\"namespace\": {}, \"registry\": {}, \"ext\": {}, \"files\": {c}}}", quote(n), quote(g), quote(e))).collect();
            let _ = writeln!(p, "  \"inventory\": [\n{}\n  ],", inv.join(",\n"));
            let dis: Vec<String> = r.disposition.iter().map(|(k, (s, n))| format!("   {}: {{\"status\": {}, \"note\": {}}}", quote(k), quote(s), quote(n))).collect();
            let _ = writeln!(p, "  \"disposition\": {{\n{}\n  }},", dis.join(",\n"));
            let fs: Vec<String> = sorted_findings(r)
                .iter()
                .map(|f| format!("   {{\"severity\": {}, \"category\": {}, \"item\": {}, \"count\": {}, \"detail\": {}, \"locations\": {}}}", quote(f.severity), quote(&f.category), quote(&f.item), f.count, quote(&f.detail), arr(f.examples.iter().cloned())))
                .collect();
            let _ = writeln!(p, "  \"findings\": [\n{}\n  ],", fs.join(",\n"));
            let _ = writeln!(
                p,
                "  \"function_reachability\": {{\"functions\": {}, \"reachable_vanilla\": {}, \"reachable_pumpkin\": {}, \"entry_vanilla\": {}, \"entry_pumpkin\": {}}},",
                r.reach.functions,
                r.reach.vanilla,
                r.reach.pumpkin,
                arr(r.reach.vanilla_entry.iter().cloned()),
                arr(r.reach.pumpkin_entry.iter().cloned())
            );
            let cu: Vec<String> = r.command_usage.iter().map(|(k, v)| format!("{}: {v}", quote(k))).collect();
            let _ = write!(p, "  \"command_usage\": {{{}}}}}", cu.join(", "));
            p
        })
        .collect();
    o.push_str(&packs.join(",\n"));
    o.push_str("\n ]\n}\n");
    o
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// Categories that answer "which Density Function / Carver / Feature / Structure / Registry items are missing".
pub const MISSING_GROUPS: &[(&str, &[&str])] = &[
    ("Density function types / refs", &["density_function", "density_function-ref", "density_function-id", "noise-id", "noise-ref", "noise_settings-id", "surface_rule"]),
    ("Carvers", &["carver", "configured_carver-id", "worldgen/configured_carver-ref"]),
    ("Features (configured / placed / nested types)", &["feature", "feature-nested-type", "placement_modifier", "configured_feature-id", "placed_feature-id", "worldgen/configured_feature-ref", "worldgen/placed_feature-ref"]),
    ("Structures", &["structure_type", "structure-id", "structure_set-id", "structure_set-ref", "structure_placement", "rt_pool_element", "rt_processor", "rt_rule_test", "rt_pos_rule_test", "rt_block_entity_modifier", "template_pool", "template_pool-rejected", "processor_list-rejected", "structure-nbt"]),
    ("Registries / biomes / dimensions", &["biome-id", "biome_source", "dimension-override", "dimension_type-override", "world_preset", "client-registry-sync"]),
    ("Gameplay data (functions, commands, recipes, loot, tags)", &["function-entrypoint", "function-tag-nested-tag", "command", "function-syntax", "recipe", "loot_entry", "loot_function", "loot_condition", "loot_number_provider", "loot_pool", "loot_table-dropped", "tag-nested-ref", "function-tag-object-entry", "function-tag-replace"]),
    ("Parse / packaging", &["json-parse", "pack.mcmeta", "packaging", "io"]),
];

pub fn to_markdown(cap: &Capabilities, results: &[PackResult], pumpkin_rev: &str) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Phase 2 — Terralith / Incendium vs Pumpkin: parse failures and missing items\n");
    let _ = writeln!(o, "> Generated by `tools/datapack-audit` against Pumpkin `{pumpkin_rev}` (`reference/pumpkin-src`). Static analysis: it simulates the named Pumpkin code paths; it does not run the server.");
    let _ = writeln!(o, "> Severity: **FAIL** = rejected/panics/compile_error or content silently dropped · **WARN** = loads but differs from vanilla · **INFO** = accepted but has no runtime effect.\n");
    for r in results {
        let _ = writeln!(o, "## {}\n", r.name);
        let t = totals(r);
        let summary: Vec<String> = t.iter().map(|(k, (d, c))| format!("{k}: {d} distinct / {c} occurrences")).collect();
        let _ = writeln!(o, "Totals — {}\n", summary.join(" · "));
        let _ = writeln!(
            o,
            "Function reachability from `#minecraft:load`/`#minecraft:tick`: {} functions in pack · reachable in vanilla **{}** · reachable in Pumpkin **{}**\n",
            r.reach.functions, r.reach.vanilla, r.reach.pumpkin
        );
        if !r.command_usage.is_empty() {
            let mut cu: Vec<(&String, &usize)> = r.command_usage.iter().collect();
            cu.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let s: Vec<String> = cu.iter().map(|(k, v)| format!("`{k}` {v}")).collect();
            let _ = writeln!(o, "Commands used by `.mcfunction` files (incl. `execute … run`): {}\n", s.join(", "));
        }
        let _ = writeln!(o, "### Registry inventory and Pumpkin disposition\n");
        let _ = writeln!(o, "| namespace | registry dir | ext | files | Pumpkin status |\n|---|---|---|---:|---|");
        for ((n, g, e), c) in &r.inventory {
            let st = r.disposition.get(g).map_or("?", |d| d.0);
            let _ = writeln!(o, "| {n} | {g} | {e} | {c} | {st} |");
        }
        let _ = writeln!(o);
        let findings = sorted_findings(r);
        for (title, cats) in MISSING_GROUPS {
            let rows: Vec<&&Finding> = findings.iter().filter(|f| cats.contains(&f.category.as_str())).collect();
            let _ = writeln!(o, "### {title}\n");
            if rows.is_empty() {
                let _ = writeln!(o, "_none_\n");
                continue;
            }
            let _ = writeln!(o, "| sev | category | item | count | first location | detail |\n|---|---|---|---:|---|---|");
            for f in rows {
                let first = f.examples.first().map_or(String::new(), |e| md_cell(e));
                let _ = writeln!(o, "| {} | {} | `{}` | {} | `{}` | {} |", f.severity, f.category, md_cell(&f.item), f.count, first, md_cell(&f.detail));
            }
            let _ = writeln!(o);
        }
        let other: Vec<&&Finding> = findings.iter().filter(|f| !MISSING_GROUPS.iter().any(|(_, c)| c.contains(&f.category.as_str()))).collect();
        if !other.is_empty() {
            let _ = writeln!(o, "### Other (overrides with no runtime effect, advancements)\n");
            let _ = writeln!(o, "| sev | category | item | count | first location |\n|---|---|---|---:|---|");
            for f in other {
                let first = f.examples.first().map_or(String::new(), |e| md_cell(e));
                let _ = writeln!(o, "| {} | {} | `{}` | {} | `{}` |", f.severity, f.category, md_cell(&f.item), f.count, first);
            }
            let _ = writeln!(o);
        }
    }
    let _ = writeln!(o, "## Pumpkin capability sets used by this audit (extracted from source)\n");
    for (k, v) in &cap.types {
        let src = cap.sources.get(k).map(|s| s.join(", ")).unwrap_or_default();
        let list: Vec<String> = v.iter().map(|x| format!("`{}`", x.trim_start_matches("minecraft:"))).collect();
        let _ = writeln!(o, "- **{k}** ({}; `{src}`): {}", v.len(), list.join(", "));
    }
    let counts: Vec<String> = cap.vanilla_ids.iter().map(|(k, v)| format!("{k}={}", v.len())).collect();
    let _ = writeln!(o, "- **compiled vanilla ids**: {}", counts.join(", "));
    let _ = writeln!(o, "- **commands** ({}): {}", cap.commands.len(), cap.commands.iter().cloned().collect::<Vec<_>>().join(", "));
    o
}
