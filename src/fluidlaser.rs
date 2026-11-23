//! Pure rules of the fluid laser base and its `industrialforegoing:laser_drill_fluid`
//! recipes.
//!
//! A fifth pure module beside [`crate::runtime`] (byte-identical, never touched),
//! [`crate::chamber`], [`crate::latex`] and [`crate::laser`]: the rules of
//! `FluidLaserBaseTile` and `LaserDrillFluidRecipe`, with no game access. Upstream
//! (`Industrial-Foregoing` `ce2852db`, read for behaviour only):
//!
//! - The base is an `IndustrialMachineTile` whose bar rises to
//!   [`BASE_MAX_PROGRESS`] and is advanced only by a laser drill (the drill's
//!   `isValidTarget` is any `ILaserBase`, so one drill charges both bases). At the top
//!   `onWork()` runs: the **first** recipe in file order whose catalyst matches the one
//!   lens slot, whose **first** valid rarity answers the depth/ biome / dimension, and
//!   whose optional entity gate is satisfied produces [`output`](FluidRecipe::amount) mB
//!   into the output tank — there is **no weighted pick** for fluids.
//! - The tank is `SidedFluidTankComponent("output", 32000, …, Action.DRAIN)`: every face
//!   may drain it, none may fill it, and the machine fills it itself with
//!   `fillForced` (the amount clamps to the capacity).
//! - A recipe with `entity_data` needs a living entity of that kind (health above
//!   [`ENTITY_MIN_HEALTH`]) in the 3×3×3 box from one below the base to two above; when
//!   its `data` is not empty the entity's own NBT must contain those values. The match
//!   takes [`ENTITY_DAMAGE`] health from the entity, which the glue applies.
//! - The base holds **no FE of its own** (`EnergyStorageComponent(0, 4, 10)`); the
//!   drill pays, as it does for the ore base.

use std::collections::BTreeMap;

use mod_api::serde_json::Value;
use mod_api::{Compound, Tag};

use crate::laser::Rarity;
use crate::runtime::Augments;

/// `FluidLaserBaseConfig.maxProgress`: the base's bar length.
pub const BASE_MAX_PROGRESS: i32 = 20;
/// `SidedFluidTankComponent("output", 32000, …)`: the output tank.
pub const TANK_CAPACITY_MB: i32 = 32_000;
/// `SidedInventoryComponent("lens", …, 1, 0)`: the single catalyst slot.
pub const CATALYST_SLOTS: usize = 1;
/// `entity.getHealth() > 5`: an entity below this is not a candidate.
pub const ENTITY_MIN_HEALTH: f32 = 5.0;
/// `candidate.hurt(candidate.damageSources().generic(), 5)`.
pub const ENTITY_DAMAGE: f32 = 5.0;

/// The optional `entity_data` gate of a recipe (`LaserDrillFluidRecipe.entityData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityGate {
    /// `entity_data.entity.type`: the entity kind's id (`minecraft:wither`).
    pub entity: String,
    /// `entity_data.data`, as compact JSON, when it is not empty: the top-level values the
    /// entity's saved compound must contain.
    pub data: Option<String>,
}

/// One `industrialforegoing:laser_drill_fluid` recipe, as its file declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FluidRecipe {
    /// `industrialforegoing:laser_drill_fluid/<name>`.
    pub id: String,
    /// `output.fluid`: the fluid key it yields.
    pub fluid: String,
    /// `output.amount`: the mB one operation adds.
    pub amount: i32,
    /// The items its `catalyst` resolves to (the lens the slot must hold).
    pub catalyst: Vec<String>,
    /// The optional nearby-entity gate.
    pub entity: Option<EntityGate>,
    /// Its `rarity` list, in file order.
    pub rarities: Vec<Rarity>,
}

impl FluidRecipe {
    /// The first rarity that answers this depth, dimension and biome, as
    /// `LaserDrillRarity.getValidRarity`'s loop returns it.
    #[must_use]
    pub fn rarity_for(
        &self,
        dimension: &str,
        depth: i32,
        in_biome_tag: &dyn Fn(&str) -> bool,
    ) -> Option<&Rarity> {
        self.rarities
            .iter()
            .find(|rarity| rarity.matches(dimension, depth, in_biome_tag))
    }

    /// Whether `lens` is this recipe's catalyst.
    #[must_use]
    pub fn catalyst_matches(&self, lens: &str) -> bool {
        self.catalyst.iter().any(|item| item == lens)
    }
}

/// The recipes whose catalyst and first valid rarity pass, in file order.
///
/// These are the **eligibility** half of `onWork` — pure reads of the lens slot, the depth, the
/// dimension type and the biome tags. The glue reads each candidate's optional entity gate only
/// after it is on this list, which is what keeps a gate's damage from preceding eligibility: an
/// ineligible recipe's gate is never looked at.
#[must_use]
pub fn place_candidates<'a>(
    recipes: &'a [FluidRecipe],
    dimension: &str,
    depth: i32,
    in_biome_tag: &dyn Fn(&str) -> bool,
    catalyst: &str,
) -> Vec<&'a FluidRecipe> {
    recipes
        .iter()
        .filter(|recipe| {
            recipe.catalyst_matches(catalyst)
                && recipe.rarity_for(dimension, depth, in_biome_tag).is_some()
        })
        .collect()
}

/// The first candidate whose gate passes: the glue applies the gate (and its damage) here, so a
/// recipe that failed eligibility is never asked.
#[must_use]
pub fn first_with_gate<'a>(
    candidates: Vec<&'a FluidRecipe>,
    gate: &mut dyn FnMut(&FluidRecipe) -> bool,
) -> Option<&'a FluidRecipe> {
    candidates.into_iter().find(|recipe| gate(recipe))
}

/// The recipe one completed bar produces: the **first** file-order recipe whose catalyst,
/// first valid rarity and `gate` all pass (`FluidLaserBaseTile.onWork`). The glue's `gate`
/// checks the recipe's optional nearby entity.
#[must_use]
pub fn choice<'a>(
    recipes: &'a [FluidRecipe],
    dimension: &str,
    depth: i32,
    in_biome_tag: &dyn Fn(&str) -> bool,
    catalyst: &str,
    gate: &mut dyn FnMut(&FluidRecipe) -> bool,
) -> Option<&'a FluidRecipe> {
    first_with_gate(
        place_candidates(recipes, dimension, depth, in_biome_tag, catalyst),
        gate,
    )
}

/// One fluid in the base's tank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tank {
    /// The fluid's id, `minecraft:empty` when the tank is empty.
    pub fluid: String,
    /// The amount, `0..=TANK_CAPACITY_MB`.
    pub mb: i32,
}

impl Tank {
    /// An empty tank, in the saved shape (`minecraft:empty`, 0 mB).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            fluid: "minecraft:empty".to_owned(),
            mb: 0,
        }
    }

    /// Whether the tank holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mb <= 0 || self.fluid == "minecraft:empty"
    }

    /// `fillForced`: add `amount` of `fluid`, clamped to the capacity. A tank holding
    /// another fluid cannot take it (the glue reads that as a refused fill).
    pub fn fill(&mut self, fluid: &str, amount: i32) {
        if amount <= 0 || (!self.is_empty() && self.fluid != fluid) {
            return;
        }
        if self.is_empty() {
            self.fluid = fluid.to_owned();
        }
        self.mb = self.mb.max(0).saturating_add(amount).min(TANK_CAPACITY_MB);
    }

    /// The room left, in mB.
    #[must_use]
    pub fn room(&self) -> i32 {
        TANK_CAPACITY_MB.saturating_sub(self.mb.max(0))
    }
}

/// The fluid base's saved state: its bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Base {
    /// The bar, `0..=bar_length`; a new base starts empty.
    pub progress: i32,
}

impl Base {
    /// A base from its saved compound; a negative bar is clamped to zero.
    #[must_use]
    pub fn from_saved(progress: i32) -> Self {
        Self {
            progress: progress.max(0),
        }
    }

    /// The bar's length with the seated augments: `floor(20 × efficiency)`, never below 1
    /// (`ProgressBarComponent.setOnStart`).
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (BASE_MAX_PROGRESS as f32 * augments.efficiency)
            .floor()
            .max(1.0) as i32
    }

    /// One advance the drill pays for: whether this one completed the bar (and `onWork`
    /// should run). The bar resets to zero when it completes.
    pub fn advance(&mut self, augments: Augments) -> bool {
        self.progress += 1;
        if self.progress >= Self::bar_length(augments) {
            self.progress = 0;
            true
        } else {
            false
        }
    }
}

/// Whether the `entity_data.data` subset `requested` (compact JSON) is contained in the
/// entity's saved compound. An empty object matches every entity; numbers compare by
/// value (NBT's float/int widths are not preserved by JSON).
#[must_use]
pub fn data_matches(requested: &str, saved: &Compound) -> bool {
    let Ok(Value::Object(wanted)) = mod_api::serde_json::from_str::<Value>(requested) else {
        return false;
    };
    wanted.iter().all(|(key, value)| {
        saved
            .get(&key.as_str().into())
            .is_some_and(|tag| tag_json(tag) == *value)
    })
}

/// One NBT value as the JSON value the recipe's `data` writes, for the subset compare.
fn tag_json(tag: &Tag) -> Value {
    match tag {
        Tag::End => Value::Null,
        Tag::Byte(value) => Value::from(*value),
        Tag::Short(value) => Value::from(*value),
        Tag::Int(value) => Value::from(*value),
        Tag::Long(value) => Value::from(*value),
        Tag::Float(bits) => {
            let value = f32::from_bits(*bits);
            if value.is_finite() {
                Value::from(f64::from(value))
            } else {
                Value::Null
            }
        }
        Tag::Double(bits) => {
            let value = f64::from_bits(*bits);
            if value.is_finite() {
                Value::from(value)
            } else {
                Value::Null
            }
        }
        Tag::String(text) => Value::String(text.to_string_lossy()),
        Tag::List(values) => Value::Array(values.iter().map(tag_json).collect()),
        Tag::Compound(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.to_string_lossy(), tag_json(value)))
                .collect(),
        ),
        Tag::ByteArray(values) => {
            Value::Array(values.iter().map(|value| Value::from(*value)).collect())
        }
        Tag::IntArray(values) => {
            Value::Array(values.iter().map(|value| Value::from(*value)).collect())
        }
        Tag::LongArray(values) => {
            Value::Array(values.iter().map(|value| Value::from(*value)).collect())
        }
    }
}

/// The recipes' lowest-level `BTreeMap` use, so a caller can build a table without
/// depending on the order of a hash map.
pub type Table = BTreeMap<String, FluidRecipe>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laser::Filter;

    fn purple() -> Vec<String> {
        vec!["industrialforegoing:purple_laser_lens".to_owned()]
    }

    fn any_biome(_tag: &str) -> bool {
        true
    }

    fn nether(tag: &str) -> bool {
        tag == "minecraft:is_nether"
    }

    /// The jar's `laser_drill_fluid/ether.json`, decoded.
    fn ether() -> FluidRecipe {
        FluidRecipe {
            id: "industrialforegoing:laser_drill_fluid/ether".to_owned(),
            fluid: "industrialforegoing:ether_gas".to_owned(),
            amount: 10,
            catalyst: purple(),
            entity: Some(EntityGate {
                entity: "minecraft:wither".to_owned(),
                data: None,
            }),
            rarities: vec![Rarity {
                biome: Filter::default(),
                dimension: Filter::default(),
                depth_min: -64,
                depth_max: 256,
                weight: 8,
            }],
        }
    }

    /// The jar's `laser_drill_fluid/lava.json`, decoded: the Nether, 5..20.
    fn lava() -> FluidRecipe {
        FluidRecipe {
            id: "industrialforegoing:laser_drill_fluid/lava".to_owned(),
            fluid: "minecraft:lava".to_owned(),
            amount: 100,
            catalyst: vec!["industrialforegoing:orange_laser_lens".to_owned()],
            entity: None,
            rarities: vec![Rarity {
                biome: Filter::default(),
                dimension: Filter {
                    whitelist: vec!["minecraft:the_nether".to_owned()],
                    blacklist: Vec::new(),
                },
                depth_min: 5,
                depth_max: 20,
                weight: 8,
            }],
        }
    }

    #[test]
    fn the_first_recipe_whose_catalyst_rarity_and_gate_pass_is_the_one_used() {
        let table = [ether(), lava()];
        let mut wither = |recipe: &FluidRecipe| {
            recipe
                .entity
                .as_ref()
                .is_some_and(|gate| gate.entity == "minecraft:wither")
        };
        let found = choice(
            &table,
            "minecraft:the_end",
            101,
            &any_biome,
            "industrialforegoing:purple_laser_lens",
            &mut wither,
        );
        assert_eq!(
            found.map(|recipe| recipe.id.as_str()),
            Some(table[0].id.as_str())
        );
        assert_eq!(
            found.map(|recipe| recipe.fluid.as_str()),
            Some("industrialforegoing:ether_gas")
        );
    }

    #[test]
    fn a_recipe_without_its_catalyst_or_its_entity_is_skipped() {
        let table = [ether(), lava()];
        let mut wither = |recipe: &FluidRecipe| {
            recipe
                .entity
                .as_ref()
                .is_some_and(|gate| gate.entity == "minecraft:wither")
        };
        assert!(
            choice(
                &table,
                "minecraft:the_end",
                101,
                &any_biome,
                "industrialforegoing:black_laser_lens",
                &mut wither
            )
            .is_none(),
            "neither recipe names the black lens"
        );
        assert!(
            choice(
                &table,
                "minecraft:the_end",
                101,
                &any_biome,
                "industrialforegoing:purple_laser_lens",
                &mut |_| false
            )
            .is_none(),
            "the ether recipe's wither is absent"
        );
    }

    /// Eligibility comes before the gate: a recipe whose catalyst or rarity fails is never asked
    /// its gate, so the damage `onWork` applies on a match cannot precede eligibility.
    #[test]
    fn an_ineligible_recipe_s_gate_is_never_asked() {
        let table = [ether(), lava()];
        let candidate = place_candidates(
            &table,
            "minecraft:the_end",
            11,
            &nether,
            "industrialforegoing:orange_laser_lens",
        );
        assert!(
            candidate.is_empty(),
            "the lava recipe is the only orange-lens recipe and the End is not the Nether"
        );
        let mut asked = Vec::new();
        let _ = first_with_gate(candidate, &mut |recipe| {
            asked.push(recipe.id.clone());
            true
        });
        assert!(
            asked.is_empty(),
            "an eligible-excluded recipe's gate was asked: {asked:?}"
        );

        // With the purple lens at a depth inside the band the ether recipe is the only candidate,
        // so its gate is the only one asked.
        let candidate = place_candidates(
            &table,
            "minecraft:the_end",
            101,
            &nether,
            "industrialforegoing:purple_laser_lens",
        );
        let mut asked = Vec::new();
        let found = first_with_gate(candidate, &mut |recipe| {
            asked.push(recipe.id.clone());
            true
        });
        assert_eq!(asked, vec![ether().id], "only the eligible recipe is asked");
        assert_eq!(
            found.map(|recipe| recipe.fluid.as_str()),
            Some("industrialforegoing:ether_gas")
        );
    }

    #[test]
    fn the_rarity_band_and_dimension_are_the_jars() {
        assert!(
            lava()
                .rarity_for("minecraft:the_nether", 10, &nether)
                .is_some()
        );
        assert!(
            lava()
                .rarity_for("minecraft:the_end", 10, &any_biome)
                .is_none(),
            "lava names the Nether"
        );
        assert!(
            lava()
                .rarity_for("minecraft:the_nether", 101, &nether)
                .is_none(),
            "lava's band is 5..20, not the base's own Y"
        );
        assert!(
            ether()
                .rarity_for("minecraft:the_end", 101, &any_biome)
                .is_some(),
            "ether answers every biome and depth"
        );
    }

    #[test]
    fn the_bar_takes_twenty_advances_and_scales_with_efficiency() {
        let mut base = Base::default();
        for _ in 0..BASE_MAX_PROGRESS - 1 {
            assert!(!base.advance(Augments::NONE));
        }
        assert_eq!(base.progress, BASE_MAX_PROGRESS - 1);
        assert!(base.advance(Augments::NONE));
        assert_eq!(base.progress, 0);
        let slow = Augments {
            speed: 1,
            efficiency: 0.5,
        };
        assert_eq!(Base::bar_length(slow), 10);
        assert_eq!(Base::bar_length(Augments::NONE), BASE_MAX_PROGRESS);
        assert_eq!(Base::from_saved(-3).progress, 0);
    }

    #[test]
    fn the_tank_adds_its_fluid_up_to_the_capacity() {
        let mut tank = Tank::empty();
        tank.fill("industrialforegoing:ether_gas", 10);
        assert_eq!(tank.mb, 10);
        assert_eq!(tank.fluid, "industrialforegoing:ether_gas");
        tank.fill("industrialforegoing:ether_gas", 10);
        assert_eq!(tank.mb, 20);
        tank.fill("industrialforegoing:ether_gas", TANK_CAPACITY_MB);
        assert_eq!(tank.mb, TANK_CAPACITY_MB);
        assert_eq!(tank.room(), 0);
        assert!(!tank.is_empty());
    }

    #[test]
    fn a_tank_holding_another_fluid_does_not_take_this_one() {
        let mut tank = Tank::empty();
        tank.fill("industrialforegoing:ether_gas", 20);
        tank.fill("minecraft:lava", 100);
        assert_eq!(tank.fluid, "industrialforegoing:ether_gas");
        assert_eq!(tank.mb, 20);
    }

    #[test]
    fn the_entity_data_subset_matches_only_the_keys_it_names() {
        let mut saved = Compound::new();
        saved.insert("Health".into(), Tag::Float(20.0_f32.to_bits()));
        saved.insert("CustomName".into(), Tag::String("Boss".into()));
        saved.insert("Silent".into(), Tag::Byte(1));
        assert!(data_matches("{}", &saved));
        assert!(data_matches(r#"{"Silent":1}"#, &saved), "byte 1 is JSON 1");
        assert!(data_matches(r#"{"Health":20.0}"#, &saved));
        assert!(
            !data_matches(r#"{"Health":10.0}"#, &saved),
            "a different value does not match"
        );
        assert!(
            !data_matches(r#"{"NoGravity":true}"#, &saved),
            "a key the entity does not carry does not match"
        );
    }

    #[test]
    fn the_constants_are_the_jars_own() {
        assert_eq!(BASE_MAX_PROGRESS, 20);
        assert_eq!(TANK_CAPACITY_MB, 32_000);
        assert_eq!(CATALYST_SLOTS, 1);
        assert_eq!(ENTITY_MIN_HEALTH, 5.0);
        assert_eq!(ENTITY_DAMAGE, 5.0);
    }
}
