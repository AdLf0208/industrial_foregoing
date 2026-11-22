//! Pure laser-drill arithmetic for the Industrial Foregoing port.
//!
//! A fourth pure module beside [`crate::runtime`] (byte-identical, never touched),
//! [`crate::chamber`] and [`crate::latex`]: the rules of `LaserDrillTile` and
//! `OreLaserBaseTile`, with no game access. Upstream (`Industrial-Foregoing` `ce2852db`,
//! Titanium `e53e869`, read for behaviour only):
//!
//! ## The laser drill
//!
//! `LaserDrillTile` is an `IndustrialAreaWorkingTile(BEHIND)`: a bar that starts **full** and
//! falls [`DRILL_MAX_PROGRESS`] ticks, and at zero runs `work()` once. `work()` looks for the
//! first laser base in the 3×3×3 box two blocks along the opposite of the drill's `facing`
//! and, when one is there and the buffer holds [`DRILL_POWER_FE`], advances that base's bar
//! by one and returns `WorkAction(1, powerPerOperation)` — the tick then takes the 1,000 FE.
//! With no base, or under 1,000 FE, it returns `WorkAction(1, 0)`: the bar restarts and
//! nothing is spent. A speed augment makes the bar fall by its value (Titanium sets
//! `progressIncrease` in `onTickWork`, so the value shows from the **second** tick) and an
//! efficiency augment shortens the bar to `floor(50 × efficiency)` after each operation.
//!
//! ## The ore laser base
//!
//! `OreLaserBaseTile` is an `IndustrialMachineTile` whose bar **rises** to
//! `floor(100 × efficiency)` and is advanced only by the drill. At the top `onWork()` runs:
//! every `industrialforegoing:laser_drill_ore` recipe whose **first** rarity satisfies its
//! depth range, its biome tag filter and its dimension-type filter becomes a candidate; its
//! weight is the rarity's weight plus [`CATALYST_MODIFIER`] for every one of the six lens
//! slots that holds the recipe's catalyst; `WeightedRandom` picks one by weight, and one
//! stack of its output goes into the fifteen output slots when there is room. The base holds
//! no FE of its own (`EnergyStorageComponent(0, 4, 10)`).
//!
//! The base's `miningDepth` field (set from its GUI, default its own Y) is the depth the
//! filters read, and the filters are the jar's: a dimension filter with an empty whitelist
//! is a blacklist; a biome filter with an empty whitelist passes every biome outside its
//! blacklist; and an unknown biome tag matches nothing (`Holder.is` on a missing tag).

use crate::runtime::Augments;

/// `LaserDrillConfig.maxStoredPower`.
pub const DRILL_CAPACITY_FE: i32 = 10_000;
/// `LaserDrillConfig.powerPerOperation`: FE one advance of a base's bar costs.
pub const DRILL_POWER_FE: i32 = 1_000;
/// `LaserDrillConfig.maxProgress`: the drill's bar length.
pub const DRILL_MAX_PROGRESS: i32 = 50;
/// `OreLaserBaseConfig.maxProgress`: the base's bar length.
pub const BASE_MAX_PROGRESS: i32 = 100;
/// `OreLaserBaseConfig.catalystModifier`: weight one matching lens slot adds.
pub const CATALYST_MODIFIER: i32 = 8;
/// `SidedInventoryComponent("lens", …, 6, 0)`: the base's six lens slots.
pub const LENS_SLOTS: usize = 6;
/// `SidedInventoryComponent("output", …, 5 * 3, 1)`: the base's fifteen output slots.
pub const OUTPUT_SLOTS: usize = 15;

/// One `whitelist`/`blacklist` pair of a rarity (`LaserDrillRarity.BiomeRarity` /
/// `DimensionRarity`). The lists hold biome **tags** for the biome filter and dimension
/// **type ids** for the dimension filter, exactly as the recipe files spell them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// `whitelist`: when it is not empty, only these match.
    pub whitelist: Vec<String>,
    /// `blacklist`: when the whitelist is empty, anything but these matches.
    pub blacklist: Vec<String>,
}

impl Filter {
    /// Whether `id` (a dimension type id) passes, with the jar's empty-whitelist rule.
    #[must_use]
    pub fn accepts_id(&self, id: &str) -> bool {
        if self.whitelist.is_empty() {
            !self.blacklist.iter().any(|listed| listed == id)
        } else {
            self.whitelist.iter().any(|listed| listed == id)
        }
    }

    /// Whether this side of a `biome_filter` passes: an empty whitelist accepts every biome
    /// no blacklisted tag names, and a non-empty one accepts when any whitelisted tag does.
    /// `in_tag` is the game's `Holder.is(TagKey)`; an unknown tag is `false`.
    #[must_use]
    pub fn accepts_tag(&self, in_tag: &dyn Fn(&str) -> bool) -> bool {
        if self.whitelist.is_empty() {
            !self.blacklist.iter().any(|tag| in_tag(tag))
        } else {
            self.whitelist.iter().any(|tag| in_tag(tag))
        }
    }
}

/// One `LaserDrillRarity` of a recipe: its filters, the depth band and its weight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rarity {
    /// `biome_filter`.
    pub biome: Filter,
    /// `dimension_filter`.
    pub dimension: Filter,
    /// `depth_min`: the lowest Y it answers.
    pub depth_min: i32,
    /// `depth_max`: the highest Y it answers.
    pub depth_max: i32,
    /// `weight`: its share of the pick.
    pub weight: i32,
}

impl Rarity {
    /// Whether this rarity answers `depth` in `dimension`, with `in_biome_tag` the game's
    /// biome-tag test (`LaserDrillRarity.getValidRarity`).
    #[must_use]
    pub fn matches(
        &self,
        dimension: &str,
        depth: i32,
        in_biome_tag: &dyn Fn(&str) -> bool,
    ) -> bool {
        self.depth_min <= depth
            && depth <= self.depth_max
            && self.dimension.accepts_id(dimension)
            && self.biome.accepts_tag(in_biome_tag)
    }
}

/// One `industrialforegoing:laser_drill_ore` recipe, as its file declares it
/// (`LaserDrillOreRecipe.CODEC`): the item or tag it yields, the lens it is weighted by and
/// the rarities it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OreRecipe {
    /// `industrialforegoing:laser_drill_ore/<name>`.
    pub id: String,
    /// The items its `output` resolves to (a `#tag` resolves to every item of the tag).
    pub output: Vec<String>,
    /// The `output` stack's count.
    pub count: i32,
    /// The items its `catalyst` resolves to (the lens that raises its weight).
    pub catalyst: Vec<String>,
    /// Its `rarity` list, in file order.
    pub rarities: Vec<Rarity>,
}

impl OreRecipe {
    /// The first rarity that answers this depth, dimension and biome, as
    /// `getValidRarity`'s loop returns it.
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

    /// The stack the recipe gives: `TagConfig.ITEM_PREFERENCE` prefers `minecraft:` first,
    /// then whatever the file (or its tag) lists first.
    #[must_use]
    pub fn output_item(&self) -> Option<&str> {
        let vanilla = self.output.iter().find(|id| id.starts_with("minecraft:"));
        vanilla.or_else(|| self.output.first()).map(String::as_str)
    }

    /// Whether `lens` is this recipe's catalyst.
    #[must_use]
    pub fn catalyst_matches(&self, lens: &str) -> bool {
        self.catalyst.iter().any(|item| item == lens)
    }
}

/// One recipe in the weighted pick, and its weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// The recipe's index in the table.
    pub recipe: usize,
    /// `rarity.weight + CATALYST_MODIFIER × matching lens slots`.
    pub weight: i32,
}

/// The candidates one `onWork()` sees: every recipe with a matching rarity, its weight
/// including the lenses in the six slots, in table order.
#[must_use]
pub fn candidates(
    recipes: &[OreRecipe],
    dimension: &str,
    depth: i32,
    in_biome_tag: &dyn Fn(&str) -> bool,
    lenses: &[Option<String>],
) -> Vec<Candidate> {
    recipes
        .iter()
        .enumerate()
        .filter_map(|(index, recipe)| {
            let rarity = recipe.rarity_for(dimension, depth, in_biome_tag)?;
            let matching = lenses
                .iter()
                .flatten()
                .filter(|lens| recipe.catalyst_matches(lens))
                .count();
            let bonus = i32::try_from(matching).unwrap_or(0) * CATALYST_MODIFIER;
            Some(Candidate {
                recipe: index,
                weight: rarity.weight + bonus,
            })
        })
        .collect()
}

/// The weighted pick `WeightedRandom.getRandomItem` makes at `roll` in `[0, 1)`.
#[must_use]
pub fn pick(candidates: &[Candidate], roll: f64) -> Option<Candidate> {
    let total: i32 = candidates
        .iter()
        .map(|candidate| candidate.weight.max(0))
        .sum();
    if total <= 0 {
        return None;
    }
    let roll = if roll.is_nan() {
        0.0
    } else {
        roll.clamp(0.0, 1.0)
    };
    let target = (roll * f64::from(total)) as i32;
    let target = target.min(total - 1);
    let mut reached = 0;
    for candidate in candidates {
        reached += candidate.weight.max(0);
        if target < reached {
            return Some(*candidate);
        }
    }
    candidates.last().copied()
}

/// What one server tick of the drill did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DrillStep {
    /// The bar fell and nothing ran.
    #[default]
    Working,
    /// The bar completed with no base in reach, or under 1,000 FE: it rests at its length
    /// and nothing is spent.
    Idle,
    /// The bar completed, a base was in reach and the FE was spent: the base's bar gains one.
    Charged,
}

/// The drill's saved state: its buffer and its bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drill {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar_length`; a new machine starts full.
    pub progress: i32,
    /// Titanium's `progressIncrease`: 1 until a speed augment sets it.
    pub increase: i32,
}

impl Default for Drill {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: DRILL_MAX_PROGRESS,
            increase: 1,
        }
    }
}

impl Drill {
    /// A drill from its saved compound; values outside the capacities are clamped and a
    /// missing bar starts full, as `ProgressBarComponent(max, max)` does.
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>) -> Self {
        Self {
            energy: energy.clamp(0, DRILL_CAPACITY_FE),
            progress: progress
                .unwrap_or(DRILL_MAX_PROGRESS)
                .clamp(0, DRILL_MAX_PROGRESS),
            increase: 1,
        }
    }

    /// The bar's length with the seated augments: `floor(50 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (DRILL_MAX_PROGRESS as f32 * augments.efficiency)
            .floor()
            .max(1.0) as i32
    }

    /// One server tick. `has_base` is whether a laser base stands in the working area.
    pub fn tick(&mut self, has_base: bool, augments: Augments) -> DrillStep {
        if self.progress > 0 {
            self.progress -= self.increase.max(1);
        }
        // Titanium's `onTickWork` sets `progressIncrease` after this tick's `setProgress`, so
        // a speed augment's value shows from the second tick.
        self.increase = augments.speed.max(1);
        if self.progress > 0 {
            return DrillStep::Working;
        }
        self.progress = Self::bar_length(augments);
        if has_base && self.energy >= DRILL_POWER_FE {
            self.energy -= DRILL_POWER_FE;
            return DrillStep::Charged;
        }
        DrillStep::Idle
    }
}

/// The ore laser base's saved state: its bar.
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

    /// The bar's length with the seated augments: `floor(100 × efficiency)`, never below 1.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `minecraft:is_mountain` as the level's biome-tag table answers it.
    fn mountain(tag: &str) -> bool {
        tag == "minecraft:is_mountain"
    }

    /// A biome in no relevant tag (the End's own, for the ten default recipes).
    fn no_tags(_tag: &str) -> bool {
        false
    }

    /// `industrialforegoing:laser_drill_ore/emerald_ore`, as the jar writes it: mountain biomes
    /// only, no dimension filter, weight 8 in its band and 1 over the whole height.
    fn emerald() -> OreRecipe {
        let biome = Filter {
            whitelist: vec!["minecraft:is_mountain".to_owned()],
            blacklist: Vec::new(),
        };
        OreRecipe {
            id: "industrialforegoing:laser_drill_ore/emerald_ore".to_owned(),
            output: vec!["minecraft:emerald_ore".to_owned()],
            count: 1,
            catalyst: vec!["industrialforegoing:lime_laser_lens".to_owned()],
            rarities: vec![
                Rarity {
                    biome: biome.clone(),
                    dimension: Filter::default(),
                    depth_min: 5,
                    depth_max: 29,
                    weight: 8,
                },
                Rarity {
                    biome,
                    dimension: Filter::default(),
                    depth_min: 0,
                    depth_max: 255,
                    weight: 1,
                },
            ],
        }
    }

    /// `coal_ore`: the End blacklisted, two bands.
    fn coal() -> OreRecipe {
        let dimension = Filter {
            whitelist: Vec::new(),
            blacklist: vec!["minecraft:the_end".to_owned()],
        };
        OreRecipe {
            id: "industrialforegoing:laser_drill_ore/coal_ore".to_owned(),
            output: vec!["minecraft:coal_ore".to_owned()],
            count: 1,
            catalyst: vec!["industrialforegoing:black_laser_lens".to_owned()],
            rarities: vec![
                Rarity {
                    biome: Filter::default(),
                    dimension: dimension.clone(),
                    depth_min: 5,
                    depth_max: 132,
                    weight: 10,
                },
                Rarity {
                    biome: Filter::default(),
                    dimension,
                    depth_min: 0,
                    depth_max: 255,
                    weight: 4,
                },
            ],
        }
    }

    /// `ancient_debris`: the Nether only.
    fn debris() -> OreRecipe {
        OreRecipe {
            id: "industrialforegoing:laser_drill_ore/ancient_debris".to_owned(),
            output: vec!["minecraft:ancient_debris".to_owned()],
            count: 1,
            catalyst: vec!["industrialforegoing:brown_laser_lens".to_owned()],
            rarities: vec![Rarity {
                biome: Filter::default(),
                dimension: Filter {
                    whitelist: vec!["minecraft:the_nether".to_owned()],
                    blacklist: Vec::new(),
                },
                depth_min: 1,
                depth_max: 20,
                weight: 1,
            }],
        }
    }

    #[test]
    fn the_first_rarity_that_matches_at_the_depth_is_the_one_used() {
        let recipe = emerald();
        assert_eq!(
            recipe
                .rarity_for("minecraft:the_end", 10, &mountain)
                .map(|rarity| rarity.weight),
            Some(8),
            "the 5..29 band comes first"
        );
        assert_eq!(
            recipe
                .rarity_for("minecraft:the_end", 101, &mountain)
                .map(|rarity| rarity.weight),
            Some(1),
            "outside 5..29 the 0..255 band answers"
        );
    }

    #[test]
    fn the_dimension_filters_are_the_jars() {
        assert!(
            coal()
                .rarity_for("minecraft:overworld", 10, &no_tags)
                .is_some()
        );
        assert!(
            coal()
                .rarity_for("minecraft:the_end", 10, &no_tags)
                .is_none(),
            "coal blacklists the End"
        );
        assert!(
            debris()
                .rarity_for("minecraft:the_end", 10, &no_tags)
                .is_none(),
            "ancient debris names the Nether"
        );
        assert!(
            debris()
                .rarity_for("minecraft:the_nether", 10, &no_tags)
                .is_some()
        );
    }

    #[test]
    fn the_biome_filter_needs_its_tag_and_the_end_biome_matches_nothing() {
        let recipes = [emerald(), coal(), debris()];
        assert!(
            candidates(&recipes, "minecraft:the_end", 101, &no_tags, &[]).is_empty(),
            "at the End's own biome none of the three answers"
        );
        let found = candidates(&recipes, "minecraft:the_end", 101, &mountain, &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].recipe, 0, "the emerald recipe is the only match");
        assert_eq!(found[0].weight, 1, "its 0..255 band weighs 1");
    }

    #[test]
    fn a_depth_outside_every_range_matches_nothing() {
        assert!(
            emerald()
                .rarity_for("minecraft:the_end", -70, &mountain)
                .is_none()
        );
        assert!(
            emerald()
                .rarity_for("minecraft:the_end", 300, &mountain)
                .is_none()
        );
    }

    #[test]
    fn a_lens_adds_eight_weight_per_matching_slot() {
        let lenses = [
            Some("industrialforegoing:lime_laser_lens".to_owned()),
            None,
            Some("industrialforegoing:blue_laser_lens".to_owned()),
            Some("industrialforegoing:lime_laser_lens".to_owned()),
            None,
            None,
        ];
        let found = candidates(&[emerald()], "minecraft:the_end", 101, &mountain, &lenses);
        assert_eq!(found[0].weight, 1 + 2 * CATALYST_MODIFIER);
        let found = candidates(&[emerald()], "minecraft:the_end", 101, &mountain, &[]);
        assert_eq!(found[0].weight, 1, "no lens, no bonus");
    }

    #[test]
    fn the_weighted_pick_follows_the_total_weight() {
        let table = [
            Candidate {
                recipe: 0,
                weight: 1,
            },
            Candidate {
                recipe: 1,
                weight: 3,
            },
        ];
        assert_eq!(pick(&table, 0.0).map(|c| c.recipe), Some(0));
        assert_eq!(pick(&table, 0.24).map(|c| c.recipe), Some(0));
        assert_eq!(pick(&table, 0.25).map(|c| c.recipe), Some(1));
        assert_eq!(pick(&table, 0.999).map(|c| c.recipe), Some(1));
        assert_eq!(pick(&table, -1.0).map(|c| c.recipe), Some(0), "clamped");
        assert_eq!(pick(&[], 0.5), None);
    }

    #[test]
    fn a_tag_output_prefers_minecraft_and_a_catalyst_matches_its_lens() {
        let mut tagged = coal();
        tagged.output = vec![
            "industrialforegoing:something".to_owned(),
            "minecraft:raw_iron".to_owned(),
        ];
        assert_eq!(tagged.output_item(), Some("minecraft:raw_iron"));
        assert!(tagged.catalyst_matches("industrialforegoing:black_laser_lens"));
        assert!(!tagged.catalyst_matches("industrialforegoing:red_laser_lens"));
    }

    #[test]
    fn the_drill_bar_falls_one_a_tick_and_charges_the_base_at_zero() {
        let mut machine = Drill {
            energy: DRILL_CAPACITY_FE,
            ..Drill::default()
        };
        for _ in 0..DRILL_MAX_PROGRESS - 1 {
            assert_eq!(machine.tick(true, Augments::NONE), DrillStep::Working);
        }
        assert_eq!(machine.progress, 1);
        assert_eq!(machine.tick(true, Augments::NONE), DrillStep::Charged);
        assert_eq!(machine.energy, DRILL_CAPACITY_FE - DRILL_POWER_FE);
        assert_eq!(
            machine.progress, DRILL_MAX_PROGRESS,
            "the bar restarts full"
        );
    }

    #[test]
    fn a_drill_without_a_base_or_under_a_thousand_fe_spends_nothing() {
        let mut alone = Drill {
            energy: DRILL_CAPACITY_FE,
            progress: 1,
            increase: 1,
        };
        assert_eq!(alone.tick(false, Augments::NONE), DrillStep::Idle);
        assert_eq!(alone.energy, DRILL_CAPACITY_FE);
        let mut poor = Drill {
            energy: DRILL_POWER_FE - 1,
            progress: 1,
            increase: 1,
        };
        assert_eq!(poor.tick(true, Augments::NONE), DrillStep::Idle);
        assert_eq!(poor.energy, DRILL_POWER_FE - 1);
        assert_eq!(poor.progress, DRILL_MAX_PROGRESS);
    }

    #[test]
    fn a_speed_augment_falls_four_a_tick_from_the_second_tick() {
        let speed = Augments {
            speed: 4,
            efficiency: 1.0,
        };
        let mut machine = Drill {
            energy: DRILL_CAPACITY_FE,
            ..Drill::default()
        };
        machine.tick(true, speed);
        assert_eq!(
            machine.progress,
            DRILL_MAX_PROGRESS - 1,
            "the first tick is one"
        );
        machine.tick(true, speed);
        assert_eq!(machine.progress, DRILL_MAX_PROGRESS - 5, "then four a tick");
        machine.tick(true, speed);
        assert_eq!(machine.progress, DRILL_MAX_PROGRESS - 9);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_drill_bar_to_forty() {
        let slow = Augments {
            speed: 1,
            efficiency: 0.8,
        };
        assert_eq!(Drill::bar_length(slow), 40);
        assert_eq!(Drill::bar_length(Augments::NONE), DRILL_MAX_PROGRESS);
        let mut machine = Drill {
            energy: DRILL_CAPACITY_FE,
            progress: 40,
            increase: 1,
        };
        for _ in 0..39 {
            assert_eq!(machine.tick(true, slow), DrillStep::Working);
        }
        assert_eq!(machine.tick(true, slow), DrillStep::Charged);
        assert_eq!(
            machine.progress, 40,
            "the bar restarts at the shortened length"
        );
    }

    #[test]
    fn the_base_bar_takes_a_hundred_advances_and_emits_once() {
        let mut base = Base::default();
        for _ in 0..BASE_MAX_PROGRESS - 1 {
            assert!(!base.advance(Augments::NONE));
        }
        assert_eq!(base.progress, BASE_MAX_PROGRESS - 1);
        assert!(
            base.advance(Augments::NONE),
            "the hundredth advance completes it"
        );
        assert_eq!(base.progress, 0);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_base_bar_to_eighty() {
        let slow = Augments {
            speed: 1,
            efficiency: 0.8,
        };
        assert_eq!(Base::bar_length(slow), 80);
        assert_eq!(Base::bar_length(Augments::NONE), BASE_MAX_PROGRESS);
        let mut base = Base::default();
        for _ in 0..79 {
            assert!(!base.advance(slow));
        }
        assert!(base.advance(slow));
        assert_eq!(base.progress, 0);
    }

    #[test]
    fn the_saved_values_are_clamped_to_the_capacities() {
        let drill = Drill::from_saved(50_000, Some(-4));
        assert_eq!(drill.energy, DRILL_CAPACITY_FE);
        assert_eq!(drill.progress, 0);
        assert_eq!(Drill::from_saved(-1, None).progress, DRILL_MAX_PROGRESS);
        assert_eq!(Base::from_saved(-9).progress, 0);
    }

    #[test]
    fn the_constants_are_the_jars_own() {
        assert_eq!(DRILL_POWER_FE, 1_000);
        assert_eq!(DRILL_MAX_PROGRESS, 50);
        assert_eq!(DRILL_CAPACITY_FE, 10_000);
        assert_eq!(BASE_MAX_PROGRESS, 100);
        assert_eq!(CATALYST_MODIFIER, 8);
        assert_eq!(LENS_SLOTS, 6);
        assert_eq!(OUTPUT_SLOTS, 15);
    }
}
