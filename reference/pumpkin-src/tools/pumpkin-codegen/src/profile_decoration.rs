use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde_json::{Value, json};

use crate::profile_resources::{canonical_id, files, read_json, resource_path};

pub fn build(data: &Path) -> io::Result<TokenStream> {
    let placed = files(data, "worldgen/placed_feature")?;
    let configured = files(data, "worldgen/feature")?;
    let mut configured_entries = Vec::new();
    let mut placed_entries = Vec::new();
    for (id, path) in &configured {
        let mut value = read_json(path);
        prepare(data, &placed, &configured, &mut value);
        let tokens = crate::configured_feature::value_to_configured_feature(&value);
        configured_entries.push(quote! {
            map.insert(pumpkin_data::configured_feature::ConfiguredFeature::Profile(#id), #tokens);
        });
    }
    for (id, path) in &placed {
        let mut value = read_json(path);
        prepare(data, &placed, &configured, &mut value);
        let tokens = crate::configured_feature::value_to_inline_placed_feature(&value);
        placed_entries.push(quote! {
            map.insert(pumpkin_data::placed_feature::PlacedFeature::Profile(#id), #tokens);
        });
    }

    let configured_tokens = emit_profile_builder(
        format_ident!("build_profile_configured_features"),
        quote!(
            pumpkin_data::configured_feature::ConfiguredFeature,
            ConfiguredFeature
        ),
        &configured_entries,
    );
    let placed_tokens = emit_profile_builder(
        format_ident!("build_profile_placed_features"),
        quote!(pumpkin_data::placed_feature::PlacedFeature, PlacedFeature),
        &placed_entries,
    );
    Ok(quote! { #configured_tokens #placed_tokens })
}

fn emit_profile_builder(
    name: syn::Ident,
    types: TokenStream,
    entries: &[TokenStream],
) -> TokenStream {
    // In debug builds each constructor can reserve stack space for its temporaries.
    // Keep the stack bounded when constructing the complete staged registry.
    let batches: TokenStream = entries
        .chunks(8)
        .enumerate()
        .map(|(index, batch)| {
            let insert = format_ident!("insert_batch_{index}");
            quote! {
                #[inline(never)]
                fn #insert(map: &mut std::collections::HashMap<#types>) {
                    #(#batch)*
                }
                #insert(&mut map);
            }
        })
        .collect();
    crate::configured_feature::emit_builder(name, types, batches)
}

fn prepare(
    data: &Path,
    placed: &BTreeMap<String, std::path::PathBuf>,
    configured: &BTreeMap<String, std::path::PathBuf>,
    value: &mut Value,
) {
    let reference = |value: &mut Value, registry: &BTreeMap<String, std::path::PathBuf>| {
        if let Some(id) = value.as_str() {
            let id = canonical_id(id);
            assert!(registry.contains_key(&id), "Missing feature {id}");
            *value = json!(format!("profile/{id}"));
        }
    };
    match value {
        Value::Object(fields) => {
            for (key, value) in fields.iter_mut() {
                if let Some(id) = value.as_str()
                    && (key.ends_with("_provider")
                        || matches!(
                            key.as_str(),
                            "provider"
                                | "ground_state"
                                | "to_place"
                                | "source"
                                | "then"
                                | "fallback"
                                | "fluid"
                                | "barrier"
                                | "block"
                        ))
                {
                    let mut seen = BTreeSet::new();
                    let mut id = id.to_owned();
                    loop {
                        let path = resource_path(data, "worldgen/block_state_provider", &id);
                        if !path.is_file() {
                            break;
                        }
                        assert!(
                            seen.insert(path.clone()),
                            "Recursive block state provider {id}"
                        );
                        *value = read_json(&path);
                        if let Some(next) = value.as_str() {
                            id = next.to_owned();
                        } else {
                            break;
                        }
                    }
                }
                if !matches!(key.as_str(), "properties" | "Properties") {
                    prepare(data, placed, configured, value);
                }
            }
            for key in ["type", "predicate_type"] {
                if let Some(id) = fields.get(key).and_then(Value::as_str) {
                    fields.insert(key.to_owned(), json!(canonical_id(id)));
                }
            }
            if let Some(y) = fields.remove("absolute") {
                fields.insert("profile_absolute".to_owned(), y);
            }

            let kind = fields
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if kind == "minecraft:sequence" {
                fields.insert("type".to_owned(), json!("pumpkin:profile_sequence"));
            }
            let target = if fields.contains_key("placement") || !fields.contains_key("type") {
                configured
            } else {
                placed
            };
            if let Some(feature) = fields.get_mut("feature") {
                reference(
                    feature,
                    if feature
                        .as_str()
                        .is_some_and(|name| target.contains_key(&canonical_id(name)))
                    {
                        target
                    } else if std::ptr::eq(target, placed) {
                        configured
                    } else {
                        placed
                    },
                );
            }
            for key in [
                "default",
                "feature_true",
                "feature_false",
                "vegetation_feature",
                "cap_feature",
            ] {
                if let Some(feature) = fields.get_mut(key)
                    && let Some(id) = feature.as_str()
                    && placed.contains_key(&canonical_id(id))
                {
                    reference(feature, placed);
                }
            }
            if let Some(features) = fields.get_mut("features").and_then(Value::as_array_mut) {
                for feature in features {
                    reference(feature, placed);
                    if kind == "minecraft:weighted_random_selector"
                        && let Some(child) = feature.get_mut("data")
                    {
                        reference(child, placed);
                    }
                }
            }

            if kind == "minecraft:matching_block_tag"
                && let Some(tag) = fields.get("tag").and_then(Value::as_str)
            {
                let blocks = block_tag(data, tag, &mut BTreeSet::new());
                fields.insert("type".into(), json!("minecraft:matching_blocks"));
                fields.insert("blocks".into(), json!(blocks));
            }
            if let Some(blocks) = fields.get_mut("blocks")
                && let Some(tag) = blocks.as_str().and_then(|id| id.strip_prefix('#'))
            {
                *blocks = json!(block_tag(data, tag, &mut BTreeSet::new()));
            }
            if fields
                .get("predicate_type")
                .or_else(|| fields.get("type"))
                .and_then(Value::as_str)
                == Some("minecraft:tag_match")
                && let Some(tag) = fields.get("tag").and_then(Value::as_str)
            {
                let rules: Vec<_> = block_tag(data, tag, &mut BTreeSet::new())
                    .into_iter()
                    .map(|block| json!({"predicate_type": "minecraft:block_match", "block": block}))
                    .collect();
                fields.insert("predicate_type".into(), json!("minecraft:any_of"));
                fields.insert("rules".into(), json!(rules));
            }
            for key in ["replaceable", "root_replaceable", "replaceable_blocks"] {
                if let Some(field) = fields.get_mut(key)
                    && let Some(tag) = field.as_str().and_then(|id| id.strip_prefix('#'))
                {
                    let blocks = block_tag(data, tag, &mut BTreeSet::new());
                    *field = json!({"type": "minecraft:matching_blocks", "blocks": blocks});
                }
            }
            for key in [
                "can_place_on",
                "can_attach_to",
                "can_grow_through",
                "muddy_roots_in",
                "grow_through",
                "place_on",
                "place_on_blocks",
            ] {
                if let Some(field) = fields.get_mut(key)
                    && let Some(tag) = field.as_str().and_then(|id| id.strip_prefix('#'))
                {
                    *field = json!(block_tag(data, tag, &mut BTreeSet::new()));
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                prepare(data, placed, configured, value);
            }
        }
        _ => {}
    }
}

fn block_tag(data: &Path, tag: &str, visiting: &mut BTreeSet<String>) -> BTreeSet<String> {
    let tag = canonical_id(tag);
    assert!(visiting.insert(tag.clone()), "Recursive block tag {tag}");
    let file = resource_path(data, "tags/block", &tag);
    let value = read_json(&file);
    let mut blocks = BTreeSet::new();
    for entry in value["values"]
        .as_array()
        .expect("Missing block tag values")
    {
        let id = entry
            .as_str()
            .or_else(|| entry["id"].as_str())
            .expect("Invalid block tag value");
        if let Some(child) = id.strip_prefix('#') {
            if entry["required"].as_bool() != Some(false)
                || resource_path(data, "tags/block", child).is_file()
            {
                blocks.extend(block_tag(data, child, visiting));
            }
        } else {
            blocks.insert(canonical_id(id));
        }
    }
    visiting.remove(&tag);
    blocks
}
