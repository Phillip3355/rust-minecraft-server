//! Item stack discovery and accounting (duplication / loss detection).
//!
//! Item stacks are located by *known container keys* rather than by shape,
//! because since 1.20.5 a stack's `count` is optional (defaults to 1), so an
//! `{id: ...}` compound is indistinguishable from an entity/block entity by
//! shape alone.

use std::collections::BTreeMap;

use crate::nbt::Tag;
use crate::report::{Category, Report};

/// Keys whose value is a single item stack compound.
const SINGLE_KEYS: &[&str] = &[
    "Item",            // item entity, item frame
    "item",            // container component entry {slot, item}, decorated pot, ...
    "RecordItem",      // jukebox
    "Book",            // lectern
    "body_armor_item", // horse/wolf armor (1.20.5+)
    "SaddleItem",
    "FireworksItem",
    "Trident",
];
/// Keys whose value is a list of item stacks.
const LIST_KEYS: &[&str] = &[
    "Items",
    "Inventory",
    "EnderItems",
    "ArmorItems",
    "HandItems",
    "minecraft:bundle_contents",
    "minecraft:charged_projectiles",
];
/// Keys whose value is a compound of slot-name -> item stack (1.21.5+ entity equipment).
const MAP_KEYS: &[&str] = &["equipment"];

pub const MAX_STACK: i64 = 99;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Totals {
    /// item id -> total count
    pub by_id: BTreeMap<String, i64>,
}

impl Totals {
    pub fn add(&mut self, id: &str, n: i64) {
        *self.by_id.entry(id.to_string()).or_insert(0) += n;
    }
    pub fn merge(&mut self, o: &Totals) {
        for (k, v) in &o.by_id {
            self.add(k, *v);
        }
    }
    pub fn total(&self) -> i64 {
        self.by_id.values().sum()
    }
}

/// Returns (id, count) if `t` looks like an item stack; validates count.
fn stack(t: &Tag, loc: &str, report: &mut Report) -> Option<(String, i64)> {
    let id = t.get("id")?.as_str()?;
    if id.is_empty() || id == "minecraft:air" {
        return None;
    }
    // 1.20.5+: "count" (int, optional, default 1). Pre-1.20.5: "Count" (byte).
    let count = match (t.get("count"), t.get("Count")) {
        (Some(c), _) | (None, Some(c)) => match c.as_i64() {
            Some(v) => v,
            None => {
                report.error(Category::ItemStack, loc, format!("{id}: count is not an integer"));
                return None;
            }
        },
        (None, None) => 1,
    };
    if count <= 0 {
        report.error(Category::ItemStack, loc, format!("{id}: non-positive stack count {count}"));
    } else if count > MAX_STACK {
        report.error(Category::ItemStack, loc, format!("{id}: stack count {count} exceeds max {MAX_STACK}"));
    }
    Some((id.to_string(), count.max(0)))
}

/// Recursively collect every item stack reachable from `t`, including stacks
/// nested in shulker boxes / bundles.
pub fn collect(t: &Tag, loc: &str, totals: &mut Totals, report: &mut Report) {
    match t {
        Tag::Compound(kv) => {
            for (k, v) in kv {
                if SINGLE_KEYS.contains(&k.as_str()) {
                    if let Some((id, n)) = stack(v, loc, report) {
                        totals.add(&id, n);
                    }
                } else if LIST_KEYS.contains(&k.as_str()) {
                    for e in v.as_list().unwrap_or(&[]) {
                        if let Some((id, n)) = stack(e, loc, report) {
                            totals.add(&id, n);
                        }
                    }
                } else if MAP_KEYS.contains(&k.as_str()) {
                    for (_, e) in v.as_compound().unwrap_or(&[]) {
                        if let Some((id, n)) = stack(e, loc, report) {
                            totals.add(&id, n);
                        }
                    }
                }
                collect(v, loc, totals, report);
            }
        }
        Tag::List(_, v) => {
            for e in v {
                collect(e, loc, totals, report);
            }
        }
        _ => {}
    }
}

/// Compare totals. Increases are reported as duplication (error); decreases
/// as possible loss (error when `strict_loss`, warning otherwise).
pub fn conserve(before: &Totals, after: &Totals, allow: &[String], strict_loss: bool, report: &mut Report) {
    let mut keys: Vec<&String> = before.by_id.keys().chain(after.by_id.keys()).collect();
    keys.sort();
    keys.dedup();
    for k in keys {
        if allow.iter().any(|a| a == k) {
            continue;
        }
        let b = before.by_id.get(k).copied().unwrap_or(0);
        let a = after.by_id.get(k).copied().unwrap_or(0);
        if a > b {
            report.error(Category::ItemDuplication, k.as_str(), format!("count increased {b} -> {a} (+{})", a - b));
        } else if a < b {
            let msg = format!("count decreased {b} -> {a} (-{})", b - a);
            if strict_loss {
                report.error(Category::ItemLoss, k.as_str(), msg);
            } else {
                report.warn(Category::ItemLoss, k.as_str(), msg);
            }
        }
    }
    report.bump("items_before", before.total());
    report.bump("items_after", after.total());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compound;

    #[test]
    fn nested_shulker_and_default_count() {
        let shulker = compound! {
            "id" => Tag::String("minecraft:shulker_box".into()),
            "components" => compound! {
                "minecraft:container" => Tag::List(10, vec![
                    compound! { "slot" => Tag::Int(0), "item" => compound! {
                        "id" => Tag::String("minecraft:diamond".into()), "count" => Tag::Int(64) } },
                ]),
            },
        };
        let player = compound! {
            "Inventory" => Tag::List(10, vec![
                compound! { "Slot" => Tag::Byte(0), "id" => Tag::String("minecraft:diamond".into()), "count" => Tag::Int(3) },
                { let mut s = shulker; if let Tag::Compound(v) = &mut s { v.push(("Slot".into(), Tag::Byte(1))); } s },
                compound! { "Slot" => Tag::Byte(2), "id" => Tag::String("minecraft:iron_sword".into()) },
            ]),
        };
        let mut t = Totals::default();
        let mut r = Report::default();
        collect(&player, "p", &mut t, &mut r);
        assert_eq!(r.errors(), 0);
        assert_eq!(t.by_id["minecraft:diamond"], 67);
        assert_eq!(t.by_id["minecraft:shulker_box"], 1);
        assert_eq!(t.by_id["minecraft:iron_sword"], 1);
    }

    #[test]
    fn invalid_counts_flagged() {
        let inv = compound! { "Items" => Tag::List(10, vec![
            compound! { "id" => Tag::String("minecraft:dirt".into()), "count" => Tag::Int(0) },
            compound! { "id" => Tag::String("minecraft:dirt".into()), "count" => Tag::Int(127) },
        ]) };
        let mut r = Report::default();
        collect(&inv, "c", &mut Totals::default(), &mut r);
        assert_eq!(r.errors(), 2);
    }

    #[test]
    fn conservation() {
        let mut a = Totals::default();
        a.add("minecraft:dirt", 10);
        a.add("minecraft:stone", 5);
        let mut b = Totals::default();
        b.add("minecraft:dirt", 11);
        b.add("minecraft:stone", 4);
        let mut r = Report::default();
        conserve(&a, &b, &[], false, &mut r);
        assert!(r.has(Category::ItemDuplication, crate::report::Severity::Error));
        assert!(r.has(Category::ItemLoss, crate::report::Severity::Warning));
        let mut r2 = Report::default();
        conserve(&a, &b, &["minecraft:dirt".into()], true, &mut r2);
        assert!(!r2.has(Category::ItemDuplication, crate::report::Severity::Error));
        assert!(r2.has(Category::ItemLoss, crate::report::Severity::Error));
    }
}
