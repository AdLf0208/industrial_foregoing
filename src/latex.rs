//! Pure latex-chain arithmetic for the Industrial Foregoing port.
//!
//! A third pure module beside [`crate::runtime`] (byte-identical, never touched) and
//! [`crate::chamber`]: the rules of `FluidExtractorTile` and `LatexProcessingUnitTile`, with no
//! game access. Upstream (`Industrial-Foregoing` `ce2852db`, Titanium `e53e869`, read for
//! behaviour only):
//!
//! ## The fluid extractor
//!
//! `FluidExtractorTile` is an `IndustrialAreaWorkingTile(BEHIND)` over Titanium's
//! `ProgressBarComponent`: a bar that starts full, falls one a tick, and at zero runs `work()`
//! once. `work()` looks at the block **behind** the machine (the range type is `BEHIND`), finds the
//! first `industrialforegoing:fluid_extractor` recipe whose `input` matches it — a named recipe
//! before the `defaultRecipe` one, which answers for any other log — and:
//!
//! - an empty or full machine does nothing (`WorkAction(1, 0)`: a full 100-tick bar, no FE);
//! - otherwise it adds `output.amount` mB, **times three with at least
//!   [`EXTRACTOR_POWER_FE`] FE**, and takes [`EXTRACTOR_POWER_FE`];
//! - with probability `breakChance` (0.01 in every recipe the jar ships) the log takes a hit: the
//!   counter shared by every extractor on that block position rises, and on the **eighth** hit it
//!   resets, the block is replaced by the recipe's `result` (a stripped log), and
//!   `output.amount * 200` mB arrive with power (`* 1` without);
//! - the next bar value is `floor(100 * workAmount)`, so the cooldown is 40 ticks when powered and
//!   100 when not.
//!
//! ## The latex processing unit
//!
//! `LatexProcessingUnitTile` is an `IndustrialProcessingTile`: a bar that rises one a tick while it
//! can work, [`UNIT_POWER_PER_TICK`] FE a tick, and at [`UNIT_MAX_PROGRESS`] it runs `onFinish`
//! once: [`UNIT_LATEX_MB`] of latex and [`UNIT_WATER_MB`] of water out, one `dryrubber` in.
//! `canIncrease()` needs both fluids, the buffer and room in the nine output slots, and a bar that
//! cannot work resets to 0 and takes nothing (Titanium's `MultiProgressBarHandler.update`).
//!
//! ## What this port cannot take from the pin
//!
//! - **No RNG on the host.** `breakChance` is rolled against the game's random upstream; here the
//!   glue supplies a roll from the game clock (`Host` has no random source at this pin), and the
//!   harness can pin the next roll so the eighth-hit path is observable in bounded time.
//! - **A mod fluid cannot be inserted** (`native_fluid_insert` takes water and lava only), so the
//!   extractor's latex leaves by draining and the unit's latex tank is filled by the command path;
//!   water reaches the unit through the real capability.

use crate::runtime::Augments;

/// `FluidExtractorConfig.maxStoredPower`.
pub const EXTRACTOR_CAPACITY_FE: i32 = 10_000;
/// `FluidExtractorConfig.powerPerOperation`: FE an operation costs.
pub const EXTRACTOR_POWER_FE: i32 = 500;
/// `FluidExtractorConfig.maxLatexTankSize` (the comment says 8,000; the value is 1,000).
pub const EXTRACTOR_TANK_MB: i32 = 1_000;
/// `FluidExtractorConfig.maxProgress`: the bar's length.
pub const EXTRACTOR_MAX_PROGRESS: i32 = 100;
/// `WorkAction(0.4f, powerPerOperation)`: the bar rests at 40 after a powered operation.
pub const EXTRACTOR_POWERED_SHARE: (i32, i32) = (2, 5);
/// `WorkAction(1f, 0)` — and Titanium's reset at a completed bar: 100 ticks.
pub const EXTRACTOR_IDLE_SHARE: (i32, i32) = (1, 1);
/// `output.getAmount() * (powered ? 3 : 1)`.
pub const EXTRACTOR_POWERED_MULTIPLIER: i32 = 3;
/// `output.getAmount() * (powered ? 200 : 1)` when the log is consumed.
pub const EXTRACTOR_BURST_MULTIPLIER: i32 = 200;
/// `if (extractionProgress.getProgress() > 7)`: the eighth hit consumes the block.
pub const EXTRACTOR_HITS: i32 = 8;
/// `LatexProcessingUnitConfig.maxStoredPower`.
pub const UNIT_CAPACITY_FE: i32 = 10_000;
/// `LatexProcessingUnitConfig.powerPerTick`.
pub const UNIT_POWER_PER_TICK: i32 = 20;
/// `LatexProcessingUnitConfig.maxLatexTankSize`.
pub const UNIT_TANK_MB: i32 = 16_000;
/// `LatexProcessingUnitTile.AMOUNT_LATEX` (the book says 75; the code drains 750).
pub const UNIT_LATEX_MB: i32 = 750;
/// `LatexProcessingUnitTile.AMOUNT_WATER` (the book says 1,000; the code drains 500).
pub const UNIT_WATER_MB: i32 = 500;
/// `getMaxProgress()` on `IndustrialProcessingTile`: the bar's length.
pub const UNIT_MAX_PROGRESS: i32 = 100;
/// `SidedInventoryComponent("output", …, 9, 2)`: the unit's nine output slots.
pub const UNIT_SLOTS: usize = 9;

/// One `industrialforegoing:fluid_extractor` recipe, as its file declares it
/// (`FluidExtractorRecipe.CODEC`): the log it answers for, the block it leaves, the chance a hit
/// consumes it, the fluid it gives and whether it is the fallback for any other log.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractorRecipe {
    /// `industrialforegoing:fluid_extractor/<name>`.
    pub id: String,
    /// The items (`Item` or `#tag`, resolved by the game) the block behind must be one of.
    pub input: Vec<String>,
    /// The `/setblock` string the block becomes on the eighth hit (`result`).
    pub result: String,
    /// `breakChance`: the chance one operation counts a hit.
    pub break_chance: f32,
    /// `output.amount`, in millibuckets, of the mod's latex.
    pub output_mb: i32,
    /// `defaultRecipe`: the fallback for any log the named recipes do not name.
    pub default_recipe: bool,
}

impl ExtractorRecipe {
    /// Whether the block `block` (an item id, as `Ingredient.test(new ItemStack(state.getBlock()))`
    /// reads it) fits this recipe.
    #[must_use]
    pub fn accepts(&self, block: &str) -> bool {
        self.input.iter().any(|item| item == block)
    }

    /// What one powered operation gives when the block is not consumed, and what the eighth hit
    /// adds (`work()`'s two `fillForced` calls).
    #[must_use]
    pub fn amounts(&self, powered: bool) -> (i32, i32) {
        if powered {
            (
                self.output_mb * EXTRACTOR_POWERED_MULTIPLIER,
                self.output_mb * EXTRACTOR_BURST_MULTIPLIER,
            )
        } else {
            (self.output_mb, self.output_mb)
        }
    }
}

/// The recipe in force for `block`, as `FluidExtractorTile.findRecipe` asks: every named recipe
/// first, in file order, then the `defaultRecipe` one.
#[must_use]
pub fn recipe_for<'a>(recipes: &'a [ExtractorRecipe], block: &str) -> Option<&'a ExtractorRecipe> {
    recipes
        .iter()
        .find(|recipe| !recipe.default_recipe && recipe.accepts(block))
        .or_else(|| {
            recipes
                .iter()
                .find(|recipe| recipe.default_recipe && recipe.accepts(block))
        })
}

/// What one extractor operation did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extraction {
    /// Latex added to the tank, the normal amount and any burst together.
    pub added_mb: i32,
    /// The part of [`Self::added_mb`] the consumed block gave (`0` when it was not consumed).
    pub burst_mb: i32,
    /// The block the consumed log leaves (`None` when the log is still there).
    pub result: Option<String>,
    /// The shared hit counter after this operation.
    pub hits: i32,
}

/// What one server tick of the extractor did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExtractorStep {
    /// The bar fell and nothing ran.
    #[default]
    Working,
    /// The bar completed and the machine had nothing to do (a full tank, or no log behind it):
    /// the bar rests at [`EXTRACTOR_MAX_PROGRESS`] and no FE is taken.
    Idle,
    /// The bar completed and one operation ran.
    Extracted(Extraction),
}

/// The extractor's saved state: the buffer, the latex tank, the bar and the position's shared hit
/// counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extractor {
    /// FE in the buffer.
    pub energy: i32,
    /// Latex in the tank, millibuckets.
    pub latex_mb: i32,
    /// The bar, `0..=EXTRACTOR_MAX_PROGRESS`; a new machine starts full.
    pub progress: i32,
    /// The hits the block behind has taken (`FluidExtractorTile.FluidExtractionProgress`).
    pub hits: i32,
}

impl Default for Extractor {
    fn default() -> Self {
        Self {
            energy: 0,
            latex_mb: 0,
            progress: EXTRACTOR_MAX_PROGRESS,
            hits: 0,
        }
    }
}

impl Extractor {
    /// A machine from its saved compound; values outside the capacities are clamped and a missing
    /// bar starts full, as `ProgressBarComponent(maxProgress, maxProgress)` does.
    #[must_use]
    pub fn from_saved(energy: i32, latex_mb: i32, progress: Option<i32>, hits: i32) -> Self {
        Self {
            energy: energy.clamp(0, EXTRACTOR_CAPACITY_FE),
            latex_mb: latex_mb.clamp(0, EXTRACTOR_TANK_MB),
            progress: progress
                .unwrap_or(EXTRACTOR_MAX_PROGRESS)
                .clamp(0, EXTRACTOR_MAX_PROGRESS),
            hits: hits.clamp(0, EXTRACTOR_HITS),
        }
    }

    /// Room left in the tank.
    #[must_use]
    pub fn tank_room(&self) -> i32 {
        (EXTRACTOR_TANK_MB - self.latex_mb).max(0)
    }

    /// One server tick. `recipe` is the recipe the block behind the machine answers (its input
    /// matched); `roll` is this operation's `breakChance` roll in `[0, 1)`, which upstream takes
    /// from the level's random.
    pub fn tick(&mut self, recipe: Option<&ExtractorRecipe>, roll: f32) -> ExtractorStep {
        if self.progress > 0 {
            self.progress -= 1;
            if self.progress > 0 {
                return ExtractorStep::Working;
            }
        }
        // `work()`: an empty cell (no recipe) or a full tank is `WorkAction(1, 0)`.
        let Some(recipe) = recipe else {
            self.progress = EXTRACTOR_MAX_PROGRESS;
            return ExtractorStep::Idle;
        };
        if self.tank_room() == 0 {
            self.progress = EXTRACTOR_MAX_PROGRESS;
            return ExtractorStep::Idle;
        }
        let powered = self.energy >= EXTRACTOR_POWER_FE;
        let (normal, burst) = recipe.amounts(powered);
        let room = self.tank_room();
        let mut added = normal.min(room).max(0);
        self.latex_mb += added;
        // `if (this.level.random.nextDouble() <= currentRecipe.breakChance)`: a hit, and the
        // eighth consumes the block.
        let mut consumed = None;
        let mut burst_added = 0;
        if roll <= recipe.break_chance {
            self.hits += 1;
        }
        if self.hits >= EXTRACTOR_HITS {
            self.hits = 0;
            burst_added = burst.min((room - added).max(0));
            self.latex_mb += burst_added;
            added += burst_added;
            consumed = Some(recipe.result.clone());
        }
        if powered {
            self.energy -= EXTRACTOR_POWER_FE;
        }
        self.progress = share(
            EXTRACTOR_MAX_PROGRESS,
            if powered {
                EXTRACTOR_POWERED_SHARE
            } else {
                EXTRACTOR_IDLE_SHARE
            },
        );
        ExtractorStep::Extracted(Extraction {
            added_mb: added,
            burst_mb: burst_added,
            result: consumed,
            hits: self.hits,
        })
    }
}

/// `floor(max * numerator / denominator)`, the bar value upstream sets from `workAmount`
/// (`0.4` powered, `1` idle).
#[must_use]
pub fn share(max: i32, (numerator, denominator): (i32, i32)) -> i32 {
    max * numerator / denominator
}

/// What one server tick of the latex processing unit did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UnitStep {
    /// The bar could not work: it is at 0 and no FE was taken.
    #[default]
    Idle,
    /// The bar rose and [`UNIT_POWER_PER_TICK`] FE was taken.
    Working,
    /// The bar completed: the glue takes the two fluids and inserts the rubber.
    Completed,
}

/// The unit's saved state: the buffer, both tanks and the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit {
    /// FE in the buffer.
    pub energy: i32,
    /// Latex in its tank, millibuckets.
    pub latex_mb: i32,
    /// Water in its tank, millibuckets.
    pub water_mb: i32,
    /// The bar, `0..=bar_length`.
    pub progress: i32,
    /// Titanium's `progressIncrease`: 1 until a speed augment sets it.
    pub increase: i32,
}

impl Default for Unit {
    fn default() -> Self {
        Self {
            energy: 0,
            latex_mb: 0,
            water_mb: 0,
            progress: 0,
            increase: 1,
        }
    }
}

impl Unit {
    /// A machine from its saved compound; values outside the capacities are clamped.
    #[must_use]
    pub fn from_saved(energy: i32, latex_mb: i32, water_mb: i32, progress: i32) -> Self {
        Self {
            energy: energy.clamp(0, UNIT_CAPACITY_FE),
            latex_mb: latex_mb.clamp(0, UNIT_TANK_MB),
            water_mb: water_mb.clamp(0, UNIT_TANK_MB),
            progress: progress.max(0),
            increase: 1,
        }
    }

    /// `canIncrease()` without the output slot: both fluids are in the tanks.
    #[must_use]
    pub fn has_fluids(&self) -> bool {
        self.latex_mb >= UNIT_LATEX_MB && self.water_mb >= UNIT_WATER_MB
    }

    /// The bar's length with the seated augments: `floor(100 * efficiency)` (Titanium's
    /// `onStart`), never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (UNIT_MAX_PROGRESS as f32 * augments.efficiency)
            .floor()
            .max(1.0) as i32
    }

    /// One server tick. `room` is whether the output slots can take one more rubber.
    pub fn tick(&mut self, room: bool, augments: Augments) -> UnitStep {
        let length = Self::bar_length(augments);
        if !room || !self.has_fluids() || self.energy < UNIT_POWER_PER_TICK {
            // `MultiProgressBarHandler.update`: a bar that cannot work resets to 0.
            self.progress = 0;
            return UnitStep::Idle;
        }
        self.progress += self.increase.max(1);
        self.energy -= UNIT_POWER_PER_TICK;
        self.increase = augments.speed.max(1);
        if self.progress < length {
            return UnitStep::Working;
        }
        // `onFinish`: the two fluids out, one rubber in.
        self.progress = 0;
        self.latex_mb -= UNIT_LATEX_MB;
        self.water_mb -= UNIT_WATER_MB;
        UnitStep::Completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Augment, AugmentType, Augments};

    /// `industrialforegoing:fluid_extractor/oak`, as the jar writes it.
    fn oak() -> ExtractorRecipe {
        ExtractorRecipe {
            id: "industrialforegoing:fluid_extractor/oak".to_owned(),
            input: vec!["minecraft:oak_log".to_owned()],
            result: "minecraft:stripped_oak_log[axis=y]".to_owned(),
            break_chance: 0.01,
            output_mb: 2,
            default_recipe: false,
        }
    }

    /// `…/default`: any log at all, one mB, and no block left behind.
    fn fallback() -> ExtractorRecipe {
        ExtractorRecipe {
            id: "industrialforegoing:fluid_extractor/default".to_owned(),
            input: vec![
                "minecraft:birch_log".to_owned(),
                "minecraft:oak_log".to_owned(),
            ],
            result: "minecraft:air".to_owned(),
            break_chance: 0.01,
            output_mb: 1,
            default_recipe: true,
        }
    }

    fn extractor(energy: i32) -> Extractor {
        Extractor {
            energy,
            progress: 1,
            ..Extractor::default()
        }
    }

    #[test]
    fn the_named_recipe_wins_over_the_default_one() {
        let recipes = [fallback(), oak()];
        let found = recipe_for(&recipes, "minecraft:oak_log").expect("oak is named");
        assert_eq!(found.id, "industrialforegoing:fluid_extractor/oak");
        assert_eq!(found.output_mb, 2);
    }

    #[test]
    fn the_default_recipe_answers_for_any_other_log() {
        let recipes = [oak(), fallback()];
        let found = recipe_for(&recipes, "minecraft:birch_log").expect("the default answers");
        assert!(found.default_recipe);
        assert_eq!(
            recipe_for(&recipes, "minecraft:stone").map(|r| r.id.clone()),
            None
        );
    }

    #[test]
    fn no_recipe_means_no_latex_and_no_power_spent() {
        let mut machine = extractor(EXTRACTOR_CAPACITY_FE);
        let step = machine.tick(None, 0.5);
        assert_eq!(step, ExtractorStep::Idle);
        assert_eq!(machine.latex_mb, 0);
        assert_eq!(machine.energy, EXTRACTOR_CAPACITY_FE);
        assert_eq!(machine.progress, EXTRACTOR_MAX_PROGRESS);
    }

    #[test]
    fn a_powered_hit_gives_three_times_the_recipe_for_five_hundred_fe() {
        let mut machine = extractor(EXTRACTOR_CAPACITY_FE);
        let step = machine.tick(Some(&oak()), 0.5);
        let ExtractorStep::Extracted(extraction) = step else {
            panic!("the bar was at 1, so the operation ran");
        };
        assert_eq!(extraction.added_mb, 6);
        assert_eq!(extraction.burst_mb, 0);
        assert_eq!(extraction.result, None);
        assert_eq!(machine.latex_mb, 6);
        assert_eq!(machine.energy, EXTRACTOR_CAPACITY_FE - EXTRACTOR_POWER_FE);
    }

    #[test]
    fn an_unpowered_hit_gives_the_recipe_alone_for_free() {
        let mut machine = extractor(EXTRACTOR_POWER_FE - 1);
        let step = machine.tick(Some(&oak()), 0.5);
        let ExtractorStep::Extracted(extraction) = step else {
            panic!("an unpowered machine still works");
        };
        assert_eq!(extraction.added_mb, 2);
        assert_eq!(machine.energy, EXTRACTOR_POWER_FE - 1, "no FE is taken");
    }

    #[test]
    fn the_cooldown_is_forty_when_powered_and_a_hundred_when_not() {
        let mut powered = extractor(EXTRACTOR_CAPACITY_FE);
        powered.tick(Some(&oak()), 0.5);
        assert_eq!(powered.progress, 40);
        let mut free = extractor(0);
        free.tick(Some(&oak()), 0.5);
        assert_eq!(free.progress, EXTRACTOR_MAX_PROGRESS);
    }

    #[test]
    fn the_bar_falls_one_a_tick_before_the_operation_runs() {
        let mut machine = Extractor {
            energy: EXTRACTOR_CAPACITY_FE,
            ..Extractor::default()
        };
        for _ in 0..EXTRACTOR_MAX_PROGRESS - 1 {
            assert_eq!(machine.tick(Some(&oak()), 0.5), ExtractorStep::Working);
        }
        assert_eq!(machine.progress, 1);
        assert_eq!(machine.latex_mb, 0);
        assert!(matches!(
            machine.tick(Some(&oak()), 0.5),
            ExtractorStep::Extracted(_)
        ));
    }

    #[test]
    fn a_full_tank_rests_at_a_full_bar_and_takes_nothing() {
        let mut machine = Extractor {
            energy: EXTRACTOR_CAPACITY_FE,
            latex_mb: EXTRACTOR_TANK_MB,
            progress: 1,
            hits: 0,
        };
        assert_eq!(machine.tick(Some(&oak()), 0.5), ExtractorStep::Idle);
        assert_eq!(machine.energy, EXTRACTOR_CAPACITY_FE);
        assert_eq!(machine.latex_mb, EXTRACTOR_TANK_MB);
        assert_eq!(machine.progress, EXTRACTOR_MAX_PROGRESS);
    }

    #[test]
    fn a_roll_under_the_break_chance_counts_a_hit_and_a_roll_over_it_does_not() {
        let mut machine = extractor(EXTRACTOR_CAPACITY_FE);
        machine.tick(Some(&oak()), 0.5);
        assert_eq!(machine.hits, 0, "0.5 is over the 0.01 chance");
        let mut machine = extractor(EXTRACTOR_CAPACITY_FE);
        machine.tick(Some(&oak()), 0.0);
        assert_eq!(machine.hits, 1);
    }

    #[test]
    fn the_eighth_hit_strips_the_log_and_bursts_two_hundred_times() {
        let mut machine = Extractor {
            energy: EXTRACTOR_CAPACITY_FE,
            progress: 1,
            hits: EXTRACTOR_HITS - 1,
            ..Extractor::default()
        };
        let ExtractorStep::Extracted(extraction) = machine.tick(Some(&oak()), 0.0) else {
            panic!("the bar was at 1");
        };
        assert_eq!(extraction.hits, 0, "the counter resets on the eighth hit");
        assert_eq!(
            extraction.result.as_deref(),
            Some("minecraft:stripped_oak_log[axis=y]")
        );
        assert_eq!(extraction.burst_mb, 400, "2 mB x 200, powered");
        assert_eq!(extraction.added_mb, 406, "the operation's own 6 mB first");
        assert_eq!(machine.latex_mb, 406);
    }

    #[test]
    fn the_burst_is_capped_by_the_tank() {
        let mut machine = Extractor {
            energy: EXTRACTOR_CAPACITY_FE,
            latex_mb: EXTRACTOR_TANK_MB - 10,
            progress: 1,
            hits: EXTRACTOR_HITS - 1,
        };
        let ExtractorStep::Extracted(extraction) = machine.tick(Some(&oak()), 0.0) else {
            panic!("the bar was at 1");
        };
        assert_eq!(machine.latex_mb, EXTRACTOR_TANK_MB);
        assert_eq!(extraction.added_mb, 10);
        assert_eq!(extraction.burst_mb, 4);
    }

    #[test]
    fn a_new_extractor_is_empty_with_a_full_bar() {
        let machine = Extractor::default();
        assert_eq!(machine.energy, 0);
        assert_eq!(machine.latex_mb, 0);
        assert_eq!(machine.progress, EXTRACTOR_MAX_PROGRESS);
        assert_eq!(machine.hits, 0);
    }

    // -----------------------------------------------------------------------
    // The latex processing unit
    // -----------------------------------------------------------------------

    fn unit(latex: i32, water: i32, energy: i32) -> Unit {
        Unit {
            energy,
            latex_mb: latex,
            water_mb: water,
            ..Unit::default()
        }
    }

    #[test]
    fn the_unit_needs_latex_water_room_and_power() {
        let full = (UNIT_LATEX_MB, UNIT_WATER_MB, UNIT_CAPACITY_FE);
        assert_eq!(
            unit(full.0, full.1, full.2).tick(true, Augments::NONE),
            UnitStep::Working
        );
        assert_eq!(
            unit(full.0 - 1, full.1, full.2).tick(true, Augments::NONE),
            UnitStep::Idle,
            "749 mB of latex is not a rubber"
        );
        assert_eq!(
            unit(full.0, full.1 - 1, full.2).tick(true, Augments::NONE),
            UnitStep::Idle,
            "499 mB of water is not a rubber"
        );
        assert_eq!(
            unit(full.0, full.1, full.2).tick(false, Augments::NONE),
            UnitStep::Idle,
            "a full output is no room"
        );
        assert_eq!(
            unit(full.0, full.1, UNIT_POWER_PER_TICK - 1).tick(true, Augments::NONE),
            UnitStep::Idle,
            "under 20 FE is no work"
        );
    }

    #[test]
    fn the_unit_bar_rises_one_a_tick_for_twenty_fe() {
        let mut machine = unit(UNIT_LATEX_MB, UNIT_WATER_MB, UNIT_CAPACITY_FE);
        for expected in 1..UNIT_MAX_PROGRESS {
            assert_eq!(machine.tick(true, Augments::NONE), UnitStep::Working);
            assert_eq!(machine.progress, expected);
        }
        assert_eq!(
            machine.energy,
            UNIT_CAPACITY_FE - UNIT_POWER_PER_TICK * (UNIT_MAX_PROGRESS - 1)
        );
    }

    #[test]
    fn a_hundred_ticks_make_one_rubber_for_seven_fifty_and_five_hundred() {
        let mut machine = unit(2 * UNIT_LATEX_MB, 3 * UNIT_WATER_MB, UNIT_CAPACITY_FE);
        let mut steps = 0;
        loop {
            steps += 1;
            if machine.tick(true, Augments::NONE) == UnitStep::Completed {
                break;
            }
            assert!(steps <= UNIT_MAX_PROGRESS, "the bar never completed");
        }
        assert_eq!(steps, UNIT_MAX_PROGRESS);
        assert_eq!(machine.latex_mb, UNIT_LATEX_MB);
        assert_eq!(machine.water_mb, 2 * UNIT_WATER_MB);
        assert_eq!(machine.progress, 0);
    }

    #[test]
    fn the_unit_speed_augment_starts_from_the_second_tick() {
        // Titanium's `onTickWork` sets `progressIncrease` after this tick's `setProgress`, so a
        // speed augment's value shows from the second tick — the chamber's measured behaviour.
        let speed = Augments {
            speed: 4,
            efficiency: 1.0,
        };
        let mut machine = unit(UNIT_LATEX_MB, UNIT_WATER_MB, UNIT_CAPACITY_FE);
        machine.tick(true, speed);
        assert_eq!(machine.progress, 1, "the first tick is still one");
        machine.tick(true, speed);
        assert_eq!(machine.progress, 5, "then four a tick");
    }

    #[test]
    fn the_efficiency_augment_shortens_the_unit_bar() {
        let slow = Augments {
            speed: 1,
            efficiency: 0.8,
        };
        assert_eq!(Unit::bar_length(slow), 80);
        assert_eq!(Unit::bar_length(Augments::NONE), UNIT_MAX_PROGRESS);
        let mut machine = unit(UNIT_LATEX_MB, UNIT_WATER_MB, UNIT_CAPACITY_FE);
        let mut steps = 0;
        loop {
            steps += 1;
            if machine.tick(true, slow) == UnitStep::Completed {
                break;
            }
            assert!(steps <= 80, "an 80-tick bar never completed");
        }
        assert_eq!(steps, 80);
    }

    #[test]
    fn a_bar_that_loses_its_fluid_resets_to_zero_and_takes_nothing() {
        let mut machine = unit(UNIT_LATEX_MB, UNIT_WATER_MB, UNIT_CAPACITY_FE);
        for _ in 0..10 {
            machine.tick(true, Augments::NONE);
        }
        assert_eq!(machine.progress, 10);
        let energy = machine.energy;
        machine.latex_mb = UNIT_LATEX_MB - 1;
        assert_eq!(machine.tick(true, Augments::NONE), UnitStep::Idle);
        assert_eq!(machine.progress, 0);
        assert_eq!(machine.energy, energy, "an idle bar takes nothing");
    }

    #[test]
    fn the_unit_round_trips_its_saved_values() {
        let machine = Unit::from_saved(50_000, 99_999, -4, -9);
        assert_eq!(machine.energy, UNIT_CAPACITY_FE);
        assert_eq!(machine.latex_mb, UNIT_TANK_MB);
        assert_eq!(machine.water_mb, 0);
        assert_eq!(machine.progress, 0);
        let extractor = Extractor::from_saved(50_000, 99_999, Some(-2), 99);
        assert_eq!(extractor.energy, EXTRACTOR_CAPACITY_FE);
        assert_eq!(extractor.latex_mb, EXTRACTOR_TANK_MB);
        assert_eq!(extractor.progress, 0);
        assert_eq!(extractor.hits, EXTRACTOR_HITS);
    }

    #[test]
    fn the_constants_are_the_jars_own() {
        // The book in the jar's lang file says 75 mB and 1,000 mB; the code drains 750 and 500,
        // and the code is what the game runs. `maxLatexTankSize`'s comment says 8,000; its value
        // is 1,000.
        assert_eq!((UNIT_LATEX_MB, UNIT_WATER_MB), (750, 500));
        assert_eq!(EXTRACTOR_TANK_MB, 1_000);
        assert_eq!(EXTRACTOR_POWER_FE, 500);
        assert_eq!(UNIT_POWER_PER_TICK, 20);
        assert_eq!(EXTRACTOR_HITS, 8);
        assert_eq!(AugmentType::Speed.name(), "Speed");
        assert_eq!(
            Augment {
                kind: AugmentType::Efficiency,
                value: 0.8
            }
            .kind,
            AugmentType::Efficiency
        );
    }
}
