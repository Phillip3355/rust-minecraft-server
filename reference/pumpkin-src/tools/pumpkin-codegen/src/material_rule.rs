use heck::ToShoutySnakeCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Deserialized block reference used in material rules.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum BlockStateCodecStruct {
    Name(String),
    Structured {
        #[serde(rename = "Name", alias = "id")]
        name: String,
        #[serde(rename = "Properties", alias = "properties")]
        #[allow(dead_code)]
        properties: Option<BTreeMap<String, String>>,
    },
}

impl ToTokens for BlockStateCodecStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = match self {
            Self::Name(name) => name,
            Self::Structured { name, .. } => name,
        };
        let name_stripped = name.strip_prefix("minecraft:").unwrap_or(name);
        let block_ident =
            quote::format_ident!("{}", name_stripped.to_uppercase().replace([':', '-'], "_"));
        tokens.extend(quote! {
            crate::Block::#block_ident.default_state
        });
    }
}

/// Deserialized Y offset that can be expressed relative to different reference points.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum YOffsetStruct {
    Absolute { absolute: i16 },
    AboveBottom { above_bottom: i8 },
    BelowTop { below_top: i8 },
}

impl ToTokens for YOffsetStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Absolute { absolute } => {
                tokens.extend(quote!(YOffset::Absolute(pumpkin_util::y_offset::Absolute { absolute: #absolute })));
            }
            Self::AboveBottom { above_bottom } => {
                tokens.extend(quote!(YOffset::AboveBottom(pumpkin_util::y_offset::AboveBottom { above_bottom: #above_bottom })));
            }
            Self::BelowTop { below_top } => {
                tokens.extend(quote!(YOffset::BelowTop(pumpkin_util::y_offset::BelowTop { below_top: #below_top })));
            }
        }
    }
}

/// Raw deserialized surface material condition from `material_condition` or inline in `material_rule`.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum RawMaterialCondition {
    Ref(String),
    Direct(DirectMaterialCondition),
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type")]
pub enum DirectMaterialCondition {
    #[serde(rename = "minecraft:biome")]
    Biome {
        #[serde(deserialize_with = "deserialize_string_or_vec")]
        biome_is: Vec<String>,
    },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: String,
        min_threshold: f64,
        max_threshold: f64,
        #[serde(default)]
        is_3d: bool,
    },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: String,
        true_at_and_below: YOffsetStruct,
        false_at_and_above: YOffsetStruct,
    },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: YOffsetStruct,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:temperature")]
    Temperature,
    #[serde(rename = "minecraft:steep")]
    Steep,
    #[serde(rename = "minecraft:not")]
    Not { invert: Box<RawMaterialCondition> },
    #[serde(rename = "minecraft:hole")]
    Hole,
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface,
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: String,
    },
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> serde::de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(el) = seq.next_element()? {
                vec.push(el);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// Raw deserialized material rule from `material_rule` folder or inline.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum RawMaterialRule {
    Ref(String),
    Direct(DirectMaterialRule),
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type")]
pub enum DirectMaterialRule {
    #[serde(rename = "minecraft:block")]
    Block { result_state: BlockStateCodecStruct },
    #[serde(rename = "minecraft:sequence")]
    Sequence { sequence: Vec<RawMaterialRule> },
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: RawMaterialCondition,
        then_run: Box<RawMaterialRule>,
    },
    #[serde(rename = "minecraft:bandlands", alias = "minecraft:badlands")]
    Badlands,
    #[serde(rename = "minecraft:ore_vein")]
    OreVein {
        ore_block: BlockStateCodecStruct,
        raw_ore_block: BlockStateCodecStruct,
        filler_block: BlockStateCodecStruct,
        raw_ore_chance: f32,
        density: String,
        richness: String,
        filler_gap: String,
    },
}

/// Deserialized surface material condition that gates a material rule.
#[derive(Deserialize, Clone)]
#[serde(tag = "type")]
pub enum MaterialConditionStruct {
    #[serde(rename = "minecraft:biome")]
    Biome {
        #[serde(deserialize_with = "deserialize_string_or_vec")]
        biome_is: Vec<String>,
    },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: String,
        min_threshold: f64,
        max_threshold: f64,
    },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: String,
        true_at_and_below: YOffsetStruct,
        false_at_and_above: YOffsetStruct,
    },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: YOffsetStruct,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:temperature")]
    Temperature,
    #[serde(rename = "minecraft:steep")]
    Steep,
    #[serde(rename = "minecraft:not")]
    Not { invert: Box<Self> },
    #[serde(rename = "minecraft:hole")]
    Hole,
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface,
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: String,
    },
}

impl ToTokens for MaterialConditionStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Biome { biome_is } => {
                let biomes = biome_is.iter().map(|b| {
                    b.strip_prefix("minecraft:")
                        .unwrap_or(b)
                        .to_shouty_snake_case()
                });
                let biome_refs: Vec<TokenStream> = biomes
                    .map(|b| {
                        let ident = format_ident!("{}", b);
                        quote!(&crate::biome::Biome::#ident)
                    })
                    .collect();

                tokens.extend(quote!(
                    MaterialCondition::Biome(BiomeMaterialCondition {
                        biome_is: &[#(#biome_refs),*],
                    })
                ));
            }
            Self::NoiseThreshold {
                noise,
                min_threshold,
                max_threshold,
            } => {
                let noise_id = quote::format_ident!(
                    "{}",
                    noise
                        .strip_prefix("minecraft:")
                        .unwrap_or(noise)
                        .to_shouty_snake_case()
                );

                tokens.extend(quote!(
                    MaterialCondition::NoiseThreshold(NoiseThresholdMaterialCondition {
                        noise: DoublePerlinNoiseParameters::#noise_id,
                        min_threshold: #min_threshold,
                        max_threshold: #max_threshold,
                    })
                ));
            }
            Self::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => {
                let bytes = md5::compute(random_name.as_bytes());
                let lo = u64::from_be_bytes(bytes[0..8].try_into().expect("incorrect length"));
                let hi = u64::from_be_bytes(bytes[8..16].try_into().expect("incorrect length"));
                tokens.extend(quote!(
                    MaterialCondition::VerticalGradient(VerticalGradientMaterialCondition {
                        random_lo: #lo,
                        random_hi: #hi,
                        true_at_and_below: #true_at_and_below,
                        false_at_and_above: #false_at_and_above,
                    })
                ));
            }
            Self::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                tokens.extend(quote!(
                    MaterialCondition::YAbove(AboveYMaterialCondition {
                        anchor: #anchor,
                        surface_depth_multiplier: #surface_depth_multiplier,
                        add_stone_depth: #add_stone_depth,
                    })
                ));
            }
            Self::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                tokens.extend(quote!(
                    MaterialCondition::Water(WaterMaterialCondition {
                        offset: #offset,
                        surface_depth_multiplier: #surface_depth_multiplier,
                        add_stone_depth: #add_stone_depth,
                    })
                ));
            }
            Self::Temperature => {
                tokens.extend(quote!(MaterialCondition::Temperature));
            }
            Self::Steep => {
                tokens.extend(quote!(MaterialCondition::Steep));
            }
            Self::Not { invert } => {
                tokens.extend(quote!(
                    MaterialCondition::Not(NotMaterialCondition {
                        invert: &#invert,
                    })
                ));
            }
            Self::Hole => {
                tokens.extend(quote!(MaterialCondition::Hole(HoleMaterialCondition)));
            }
            Self::AbovePreliminarySurface => {
                tokens.extend(quote!(MaterialCondition::AbovePreliminarySurface(
                    SurfaceMaterialCondition
                )));
            }
            Self::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                surface_type,
            } => {
                let surface_type_token = match surface_type.as_str() {
                    "ceiling" => quote!(
                        pumpkin_util::math::vertical_surface_type::VerticalSurfaceType::Ceiling
                    ),
                    "floor" => quote!(
                        pumpkin_util::math::vertical_surface_type::VerticalSurfaceType::Floor
                    ),
                    _ => quote!(panic!("Unknown surface type")),
                };

                tokens.extend(quote!(
                    MaterialCondition::StoneDepth(StoneDepthMaterialCondition {
                        offset: #offset,
                        add_surface_depth: #add_surface_depth,
                        secondary_depth_range: #secondary_depth_range,
                        surface_type: #surface_type_token,
                    })
                ));
            }
        }
    }
}

/// Deserialized surface material rule that determines which block to place at a given surface point.
#[derive(Deserialize, Clone)]
#[serde(tag = "type")]
pub enum MaterialRuleStruct {
    #[serde(rename = "minecraft:block")]
    Block { result_state: BlockStateCodecStruct },
    #[serde(rename = "minecraft:sequence")]
    Sequence { sequence: Vec<Self> },
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: MaterialConditionStruct,
        then_run: Box<Self>,
    },
    #[serde(rename = "minecraft:bandlands")]
    Badlands,
    #[serde(rename = "minecraft:ore_vein")]
    OreVein {
        ore_block: BlockStateCodecStruct,
        raw_ore_block: BlockStateCodecStruct,
        filler_block: BlockStateCodecStruct,
        raw_ore_chance: f32,
        min_y: i32,
        max_y: i32,
        is_iron: bool,
    },
}

impl ToTokens for MaterialRuleStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Block { result_state } => {
                tokens.extend(quote!(
                    MaterialRule::Block(BlockMaterialRule {
                        result_state: #result_state
                    })
                ));
            }
            Self::Sequence { sequence } => {
                tokens.extend(quote!(
                    MaterialRule::Sequence(SequenceMaterialRule {
                        sequence: &[#(#sequence),*]
                    })
                ));
            }
            Self::Condition { if_true, then_run } => {
                tokens.extend(quote!(
                    MaterialRule::Condition(ConditionMaterialRule {
                        if_true: #if_true,
                        then_run: &#then_run
                    })
                ));
            }
            Self::Badlands => {
                tokens.extend(quote!(MaterialRule::Badlands(BadLandsMaterialRule)));
            }
            Self::OreVein {
                ore_block,
                raw_ore_block,
                filler_block,
                raw_ore_chance,
                min_y,
                max_y,
                is_iron,
            } => {
                tokens.extend(quote!(
                    MaterialRule::OreVein(OreVeinMaterialRule {
                        ore_block: #ore_block,
                        raw_ore_block: #raw_ore_block,
                        filler_block: #filler_block,
                        raw_ore_chance: #raw_ore_chance,
                        min_y: #min_y,
                        max_y: #max_y,
                        is_iron: #is_iron,
                    })
                ));
            }
        }
    }
}

struct MaterialResources<'a> {
    data: &'a std::path::Path,
    profile_noises: Option<&'a BTreeMap<String, std::path::PathBuf>>,
    extreme: bool,
}

impl MaterialResources<'_> {
    fn read(&self, registry: &str, id: &str) -> serde_json::Value {
        let path = crate::profile_resources::resource_path(self.data, registry, id);
        let mut value = crate::profile_resources::read_json(&path);
        if let Some(noises) = self.profile_noises {
            crate::profile_resources::profile_noise_references(&mut value, "profile", noises);
        }
        if self.extreme {
            crate::extreme_height::map_height_anchors(&mut value);
        }
        value
    }
}

fn resolve_condition(
    raw: &RawMaterialCondition,
    resources: &MaterialResources<'_>,
) -> MaterialConditionStruct {
    match raw {
        RawMaterialCondition::Ref(id) => {
            let parsed = serde_json::from_value(resources.read("worldgen/material_condition", id))
                .unwrap_or_else(|error| panic!("Invalid material condition {id}: {error}"));
            resolve_condition(&parsed, resources)
        }
        RawMaterialCondition::Direct(direct) => match direct {
            DirectMaterialCondition::Biome { biome_is } => MaterialConditionStruct::Biome {
                biome_is: biome_is.clone(),
            },
            DirectMaterialCondition::NoiseThreshold {
                noise,
                min_threshold,
                max_threshold,
                is_3d: _,
            } => MaterialConditionStruct::NoiseThreshold {
                noise: noise.clone(),
                min_threshold: *min_threshold,
                max_threshold: *max_threshold,
            },
            DirectMaterialCondition::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => MaterialConditionStruct::VerticalGradient {
                random_name: random_name.clone(),
                true_at_and_below: true_at_and_below.clone(),
                false_at_and_above: false_at_and_above.clone(),
            },
            DirectMaterialCondition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => MaterialConditionStruct::YAbove {
                anchor: anchor.clone(),
                surface_depth_multiplier: *surface_depth_multiplier,
                add_stone_depth: *add_stone_depth,
            },
            DirectMaterialCondition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => MaterialConditionStruct::Water {
                offset: *offset,
                surface_depth_multiplier: *surface_depth_multiplier,
                add_stone_depth: *add_stone_depth,
            },
            DirectMaterialCondition::Temperature => MaterialConditionStruct::Temperature,
            DirectMaterialCondition::Steep => MaterialConditionStruct::Steep,
            DirectMaterialCondition::Not { invert } => MaterialConditionStruct::Not {
                invert: Box::new(resolve_condition(invert, resources)),
            },
            DirectMaterialCondition::Hole => MaterialConditionStruct::Hole,
            DirectMaterialCondition::AbovePreliminarySurface => {
                MaterialConditionStruct::AbovePreliminarySurface
            }
            DirectMaterialCondition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                surface_type,
            } => MaterialConditionStruct::StoneDepth {
                offset: *offset,
                add_surface_depth: *add_surface_depth,
                secondary_depth_range: *secondary_depth_range,
                surface_type: surface_type.clone(),
            },
        },
    }
}

fn resolve_rule(
    raw: &RawMaterialRule,
    resources: &MaterialResources<'_>,
) -> Option<MaterialRuleStruct> {
    match raw {
        RawMaterialRule::Ref(id) => {
            let parsed = serde_json::from_value(resources.read("worldgen/material_rule", id))
                .unwrap_or_else(|error| panic!("Invalid material rule {id}: {error}"));
            resolve_rule(&parsed, resources)
        }
        RawMaterialRule::Direct(direct) => match direct {
            DirectMaterialRule::Block { result_state } => Some(MaterialRuleStruct::Block {
                result_state: result_state.clone(),
            }),
            DirectMaterialRule::Sequence { sequence } => {
                let resolved_seq: Vec<MaterialRuleStruct> = sequence
                    .iter()
                    .filter_map(|r| resolve_rule(r, resources))
                    .collect();
                Some(MaterialRuleStruct::Sequence {
                    sequence: resolved_seq,
                })
            }
            DirectMaterialRule::Condition { if_true, then_run } => {
                let cond = resolve_condition(if_true, resources);
                let resolved_then = resolve_rule(then_run, resources)?;
                Some(MaterialRuleStruct::Condition {
                    if_true: cond,
                    then_run: Box::new(resolved_then),
                })
            }
            DirectMaterialRule::Badlands => Some(MaterialRuleStruct::Badlands),
            DirectMaterialRule::OreVein {
                ore_block,
                raw_ore_block,
                filler_block,
                raw_ore_chance,
                density,
                ..
            } => {
                let is_iron = density.contains("iron");
                let (min_y, max_y) = if is_iron { (-60, -8) } else { (0, 50) };
                Some(MaterialRuleStruct::OreVein {
                    ore_block: ore_block.clone(),
                    raw_ore_block: raw_ore_block.clone(),
                    filler_block: filler_block.clone(),
                    raw_ore_chance: *raw_ore_chance,
                    min_y,
                    max_y,
                    is_iron,
                })
            }
        },
    }
}

pub fn build_profile(
    data: &std::path::Path,
    overworld: &serde_json::Value,
    nether: &serde_json::Value,
    noises: &BTreeMap<String, std::path::PathBuf>,
) -> TokenStream {
    [
        ("EXTREME_OVERWORLD_SURFACE", overworld, true),
        ("INCENDIUM_NETHER_SURFACE", nether, false),
    ]
    .into_iter()
    .map(|(name, settings, extreme)| {
        let resources = MaterialResources {
            data,
            profile_noises: Some(noises),
            extreme,
        };
        let mut value = settings
            .get("surface_rule")
            .or_else(|| settings.get("material_rule"))
            .expect("Missing profile surface rule")
            .clone();
        crate::profile_resources::profile_noise_references(&mut value, "profile", noises);
        if extreme {
            crate::extreme_height::map_height_anchors(&mut value);
        }
        let raw: RawMaterialRule =
            serde_json::from_value(value).expect("Invalid inline surface rule");
        let rule = resolve_rule(&raw, &resources).expect("Cannot resolve profile material rule");
        let ident = format_ident!("{name}");
        quote! { pub const #ident: MaterialRule = #rule; }
    })
    .collect()
}

/// Reads material_rule files and resolves conditions from material_condition folder.
pub fn build() -> TokenStream {
    let resources = MaterialResources {
        data: std::path::Path::new("../../assets/datapack/data"),
        profile_noises: None,
        extreme: false,
    };

    let top_level_rules = [
        "bedrock_floor",
        "bedrock_roof",
        "end",
        "nether",
        "overworld",
        "overworld_caves",
        "overworld_floating_islands",
    ];

    let mut const_defs = TokenStream::new();

    for rule_name in top_level_rules {
        let raw = RawMaterialRule::Ref(format!("minecraft:{}", rule_name));
        let resolved = resolve_rule(&raw, &resources)
            .unwrap_or_else(|| panic!("Failed to resolve material rule {}", rule_name));

        let const_ident = format_ident!("{}", rule_name.to_uppercase());
        const_defs.extend(quote!(
            pub const #const_ident: MaterialRule = #resolved;
        ));
    }

    quote!(
        use crate::chunk::DoublePerlinNoiseParameters;
        use crate::BlockState;
        use crate::biome::Biome;
        use crate::dimension::Dimension;
        use pumpkin_util::y_offset::YOffset;
        use pumpkin_util::y_offset::Absolute;

        pub struct BlockMaterialRule {
            pub result_state: &'static BlockState,
        }

        pub struct SequenceMaterialRule {
            pub sequence: &'static [MaterialRule],
        }

        pub struct ConditionMaterialRule {
            pub if_true: MaterialCondition,
            pub then_run: &'static MaterialRule,
        }

        pub struct BadLandsMaterialRule;

        pub struct OreVeinMaterialRule {
            pub ore_block: &'static BlockState,
            pub raw_ore_block: &'static BlockState,
            pub filler_block: &'static BlockState,
            pub raw_ore_chance: f32,
            pub min_y: i32,
            pub max_y: i32,
            pub is_iron: bool,
        }

        pub enum MaterialRule {
            Block(BlockMaterialRule),
            Sequence(SequenceMaterialRule),
            Condition(ConditionMaterialRule),
            Badlands(BadLandsMaterialRule),
            OreVein(OreVeinMaterialRule),
        }

        impl MaterialRule {
            #[must_use]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    "overworld" | "amplified" | "large_biomes" => Some(&OVERWORLD),
                    "nether" => Some(&NETHER),
                    "end" => Some(&END),
                    "bedrock_floor" => Some(&BEDROCK_FLOOR),
                    "bedrock_roof" => Some(&BEDROCK_ROOF),
                    "overworld_caves" => Some(&OVERWORLD_CAVES),
                    "overworld_floating_islands" => Some(&OVERWORLD_FLOATING_ISLANDS),
                    _ => None,
                }
            }

            #[must_use]
            pub fn from_dimension(dimension: &Dimension) -> &'static Self {
                if dimension == &Dimension::OVERWORLD {
                    &OVERWORLD
                } else if dimension == &Dimension::THE_NETHER {
                    &NETHER
                } else {
                    &END
                }
            }
        }

        pub struct BiomeMaterialCondition {
            pub biome_is: &'static [&'static Biome],
        }

        pub struct NoiseThresholdMaterialCondition {
            pub noise: DoublePerlinNoiseParameters,
            pub min_threshold: f64,
            pub max_threshold: f64,
        }

        pub struct VerticalGradientMaterialCondition {
            pub random_lo: u64,
            pub random_hi: u64,
            pub true_at_and_below: YOffset,
            pub false_at_and_above: YOffset,
        }

        pub struct AboveYMaterialCondition {
            pub anchor: YOffset,
            pub surface_depth_multiplier: i32,
            pub add_stone_depth: bool,
        }

        pub struct WaterMaterialCondition {
            pub offset: i32,
            pub surface_depth_multiplier: i32,
            pub add_stone_depth: bool,
        }

        pub struct HoleMaterialCondition;

        pub struct NotMaterialCondition {
            pub invert: &'static MaterialCondition,
        }

        pub struct SurfaceMaterialCondition;

        pub struct StoneDepthMaterialCondition {
            pub offset: i32,
            pub add_surface_depth: bool,
            pub secondary_depth_range: i32,
            pub surface_type: pumpkin_util::math::vertical_surface_type::VerticalSurfaceType,
        }

        pub enum MaterialCondition {
            Biome(BiomeMaterialCondition),
            NoiseThreshold(NoiseThresholdMaterialCondition),
            VerticalGradient(VerticalGradientMaterialCondition),
            YAbove(AboveYMaterialCondition),
            Water(WaterMaterialCondition),
            Temperature,
            Steep,
            Not(NotMaterialCondition),
            Hole(HoleMaterialCondition),
            AbovePreliminarySurface(SurfaceMaterialCondition),
            StoneDepth(StoneDepthMaterialCondition),
        }

        #const_defs
    )
}
