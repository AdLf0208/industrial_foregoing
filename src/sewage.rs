//! The sewage chain's rules: the **Sewer** (`SewerTile`) and the **Sewage Composter**
//! (`SewageComposterTile`), pure and unit-tested. The glue (`crate::sewernet`) reads the game and
//! writes these numbers back; `src/runtime.rs` stays byte-identical.
//!
//! Upstream (`Buuz135/Industrial-Foregoing` at `ce2852db`, read for behaviour):
//!
//! - `SewerTile` is an `IndustrialAreaWorkingTile(RangeType.TOP)`: a bar counting **down** from
//!   `SewerConfig.maxProgress` **100**; at zero it counts the `Animal`s in the block **above** and,
//!   when there is at least one and the buffer holds `powerPerOperation * animals`, fills the
//!   sewage tank with **`50 * animals` mB** and charges `powerPerOperation * (animals + 1)` FE
//!   (`++amount` sits inside the `if`, which the config comment itself documents as
//!   "Calculated as [VALUE * (amount + 1)]"). An empty pen answers `WorkAction(1, 0)`.
//! - `SewageComposterTile` is an `IndustrialProcessingTile`: a bar rising to
//!   `SewageComposterConfig.maxProgress` **100**, **40 FE a tick**, a fill-only 8,000 mB sewage
//!   tank and a 12-slot output-only fertilizer container. `canIncrease` needs **at least 1,000 mB**
//!   and a free output slot; `onFinish` drains **1,000 mB** and inserts **one** fertilizer.

use crate::runtime::Augments;

/// `SewerConfig.maxProgress`.
pub const SEWER_MAX_PROGRESS: i32 = 100;
/// `SewerConfig.powerPerOperation`, charged `× (animals + 1)` on a successful attempt.
pub const SEWER_POWER_PER_OPERATION: i32 = 10;
/// `SewerConfig.maxStoredPower` and `SewageComposterConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// Both tanks: `SewerConfig.maxSewageTankSize` and `SewageComposterConfig.maxTankSize`.
pub const TANK_MB: i32 = 8_000;
/// `SewerTile.work()`: `new FluidStack(SEWAGE, 50 * amount)`.
pub const SEWAGE_PER_ANIMAL: i32 = 50;
/// `SewageComposterConfig.maxProgress`.
pub const COMPOSTER_MAX_PROGRESS: i32 = 100;
/// `SewageComposterConfig.powerPerTick`.
pub const COMPOSTER_POWER_PER_TICK: i32 = 40;
/// `SewageComposterTile.canIncrease`/`onFinish`: 1,000 mB of sewage per operation.
pub const COMPOST_SEWAGE_MB: i32 = 1_000;
/// `onFinish`: `new ItemStack(ModuleCore.FERTILIZER.get())` — one.
pub const FERTILIZER_COUNT: i32 = 1;
/// `SidedInventoryComponent("fertilizer", 90, 22, 12, 1)`.
pub const OUTPUT_SLOTS: usize = 12;
/// The fluid this chain produces and consumes (`ModuleCore.SEWAGE`).
pub const SEWAGE: &str = "industrialforegoing:sewage";
/// The item the composter makes (`ModuleCore.FERTILIZER`).
pub const FERTILIZER: &str = "industrialforegoing:fertilizer";

/// What one sewer tick did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SewerStep {
    /// The bar is still falling.
    Working,
    /// The bar reached zero but the pen was empty: no FE and a full bar.
    Idle,
    /// One attempt ran: `animals` were found, `mb` mB were added (clamped), `fe` were spent.
    Collect {
        /// The number of animals in the block above.
        animals: i32,
        /// Sewage actually added to the tank.
        mb: i32,
        /// FE the `WorkAction` took.
        fe: i32,
    },
}

/// The sewer's saved numbers: buffer, bar and sewage tank.
#[derive(Debug, Clone, PartialEq)]
pub struct Sewer {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; it counts **down** and a fresh machine starts full.
    pub progress: i32,
    /// mB of `industrialforegoing:sewage` in the tank.
    pub sewage_mb: i32,
}

impl Default for Sewer {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: SEWER_MAX_PROGRESS,
            sewage_mb: 0,
        }
    }
}

impl Sewer {
    /// A machine from its save: values clamp to the capacities; a bar the save never wrote starts
    /// full (`ProgressBarComponent(maxProgress, maxProgress)`).
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>, sewage_mb: i32) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress
                .unwrap_or(SEWER_MAX_PROGRESS)
                .clamp(0, SEWER_MAX_PROGRESS),
            sewage_mb: sewage_mb.clamp(0, TANK_MB),
        }
    }

    /// The bar's length with the seated augments: `floor(100 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (SEWER_MAX_PROGRESS as f32 * augments.efficiency)
            .floor()
            .max(1.0) as i32
    }

    /// One server tick: fall the bar by the speed-augmented step, and at zero run one attempt over
    /// `animals` (the count in the block above).
    pub fn tick(&mut self, augments: Augments, animals: i32) -> SewerStep {
        let bar = Self::bar_length(augments);
        // A decreasing bar: the speed augment is this tick's step.
        self.progress = (self.progress - augments.speed.max(1)).max(0);
        if self.progress > 0 {
            return SewerStep::Working;
        }
        let animals = animals.max(0);
        if animals == 0 {
            // `WorkAction(1, powerPerOperation * 0)`: an empty pen spends nothing.
            self.progress = bar;
            return SewerStep::Idle;
        }
        let need = SEWER_POWER_PER_OPERATION * animals;
        let (mb, fe) = if self.energy < need {
            // `hasEnergy(need)` is false: the fill and the `++amount` are both skipped, but the
            // `WorkAction` still asks for `need` FE and Titanium drains what the buffer has.
            let drained = self.energy;
            self.energy = 0;
            (0, drained)
        } else {
            // `fillForced(50 * amount)` clamps at the tank's capacity; the `++amount` inside the
            // `if` is why a successful attempt costs `(animals + 1)` operations of FE.
            let mb = self.fill(SEWAGE_PER_ANIMAL * animals);
            let fe = SEWER_POWER_PER_OPERATION * (animals + 1);
            self.energy -= fe;
            (mb, fe)
        };
        // `WorkAction(1, …)`: `floor(bar * 1)` — the bar rests full.
        self.progress = bar;
        SewerStep::Collect { animals, mb, fe }
    }

    /// `fillForced`'s clamp: how much of `mb` actually fits in the tank.
    pub fn fill(&mut self, mb: i32) -> i32 {
        let taken = mb.max(0).min(TANK_MB - self.sewage_mb);
        if taken > 0 {
            self.sewage_mb += taken;
        }
        taken
    }
}

/// What one composter tick did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposterStep {
    /// No room, no sewage or no FE: the bar resets and nothing is taken.
    Idle,
    /// The bar rose and `COMPOSTER_POWER_PER_TICK` FE were spent.
    Working,
    /// The bar completed: the glue drains `mb` and inserts `count` fertilizer.
    Done {
        /// The 1,000 mB of sewage consumed.
        mb: i32,
        /// The one fertilizer made.
        count: i32,
    },
}

/// The composter's saved numbers: buffer, bar and sewage tank.
#[derive(Debug, Clone, PartialEq)]
pub struct Composter {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; it counts **up** from zero.
    pub progress: i32,
    /// mB of sewage in the fill-only tank.
    pub sewage_mb: i32,
    /// Titanium's `progressIncrease`: last tick's rate, 1 until a speed augment sets it.
    pub increase: i32,
}

impl Default for Composter {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: 0,
            sewage_mb: 0,
            increase: 1,
        }
    }
}

impl Composter {
    /// A machine from its save: values clamp to the capacities.
    #[must_use]
    pub fn from_saved(energy: i32, progress: i32, sewage_mb: i32) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.max(0),
            sewage_mb: sewage_mb.clamp(0, TANK_MB),
            increase: 1,
        }
    }

    /// The bar's length with the seated augments: `floor(100 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (COMPOSTER_MAX_PROGRESS as f32 * augments.efficiency)
            .floor()
            .max(1.0) as i32
    }

    /// One server tick. `accepts` is `canIncrease`'s output-slot half (the tank half is
    /// `sewage_mb >= 1_000`): a bar that cannot work resets to 0 and takes no FE.
    pub fn tick(&mut self, augments: Augments, accepts: bool) -> ComposterStep {
        let length = Self::bar_length(augments);
        // `canIncrease()` is the tank half (this method's caller supplies the output-slot half)
        // and `hasEnergy(powerPerTick)`: a bar that cannot work resets to 0 and takes nothing.
        if !accepts || self.sewage_mb < COMPOST_SEWAGE_MB || self.energy < COMPOSTER_POWER_PER_TICK
        {
            self.progress = 0;
            return ComposterStep::Idle;
        }
        // Titanium's `tickBar` rises by the previous tick's `progressIncrease`; `onTickWork`
        // sets this tick's from the speed augment *after* the bar has moved.
        self.progress += self.increase.max(1);
        self.energy -= COMPOSTER_POWER_PER_TICK;
        self.increase = augments.speed.max(1);
        if self.progress < length {
            return ComposterStep::Working;
        }
        // `tickBar` resets, then `onFinish`: −1,000 mB and +1 fertilizer.
        self.progress = 0;
        self.sewage_mb -= COMPOST_SEWAGE_MB;
        ComposterStep::Done {
            mb: COMPOST_SEWAGE_MB,
            count: FERTILIZER_COUNT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Augment, AugmentType};

    fn speed(value: f32) -> Augments {
        Augments::from_slots([
            Some(Augment {
                kind: AugmentType::Speed,
                value,
            }),
            None,
        ])
    }

    fn efficiency(value: f32) -> Augments {
        Augments::from_slots([
            Some(Augment {
                kind: AugmentType::Efficiency,
                value,
            }),
            None,
        ])
    }

    /// A sewer whose bar is one tick from the attempt.
    fn ready(energy: i32, sewage_mb: i32) -> Sewer {
        Sewer {
            energy,
            progress: 1,
            sewage_mb,
        }
    }

    #[test]
    fn the_constants_are_the_two_configs_own() {
        assert_eq!(SEWER_MAX_PROGRESS, 100);
        assert_eq!(SEWER_POWER_PER_OPERATION, 10);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(TANK_MB, 8_000);
        assert_eq!(SEWAGE_PER_ANIMAL, 50);
        assert_eq!(COMPOSTER_MAX_PROGRESS, 100);
        assert_eq!(COMPOSTER_POWER_PER_TICK, 40);
        assert_eq!(COMPOST_SEWAGE_MB, 1_000);
        assert_eq!(FERTILIZER_COUNT, 1);
        assert_eq!(OUTPUT_SLOTS, 12);
        assert_eq!(SEWAGE, "industrialforegoing:sewage");
        assert_eq!(FERTILIZER, "industrialforegoing:fertilizer");
    }

    #[test]
    fn one_cow_gives_fifty_mb_and_costs_twenty_fe() {
        let mut sewer = ready(10_000, 0);
        assert_eq!(
            sewer.tick(Augments::NONE, 1),
            SewerStep::Collect {
                animals: 1,
                mb: 50,
                fe: 20
            },
            "50 * 1 mB, and 10 * (1 + 1) FE"
        );
        assert_eq!(sewer.sewage_mb, 50);
        assert_eq!(sewer.energy, 9_980);
        assert_eq!(sewer.progress, 100, "WorkAction(1, …) rests the full bar");
    }

    #[test]
    fn three_animals_give_one_hundred_fifty_mb_for_forty_fe() {
        let mut sewer = ready(10_000, 0);
        assert_eq!(
            sewer.tick(Augments::NONE, 3),
            SewerStep::Collect {
                animals: 3,
                mb: 150,
                fe: 40
            }
        );
        assert_eq!(sewer.sewage_mb, 150);
        assert_eq!(sewer.energy, 9_960);
    }

    #[test]
    fn an_empty_pen_gives_nothing_and_spends_no_fe() {
        let mut sewer = ready(10_000, 0);
        assert_eq!(sewer.tick(Augments::NONE, 0), SewerStep::Idle);
        assert_eq!(sewer.sewage_mb, 0);
        assert_eq!(sewer.energy, 10_000, "no animals, 0 FE");
        assert_eq!(sewer.progress, 100);
    }

    #[test]
    fn a_buffer_under_the_per_animal_gate_produces_nothing() {
        let mut sewer = ready(9, 0);
        let step = sewer.tick(Augments::NONE, 1);
        assert_eq!(
            step,
            SewerStep::Collect {
                animals: 1,
                mb: 0,
                fe: 9
            },
            "hasEnergy(10) fails: the fill and the ++amount are skipped, and the WorkAction drains what is left"
        );
        assert_eq!(sewer.sewage_mb, 0);
        assert_eq!(sewer.energy, 0);
    }

    #[test]
    fn a_full_tank_still_spends_the_fe() {
        let mut sewer = ready(10_000, TANK_MB);
        assert_eq!(
            sewer.tick(Augments::NONE, 1),
            SewerStep::Collect {
                animals: 1,
                mb: 0,
                fe: 20
            }
        );
        assert_eq!(sewer.sewage_mb, TANK_MB);
        assert_eq!(sewer.energy, 9_980);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_faster() {
        let mut sewer = Sewer::from_saved(0, None, 0);
        assert_eq!(sewer.tick(speed(4.0), 0), SewerStep::Working);
        assert_eq!(sewer.progress, 96);
        assert_eq!(Sewer::bar_length(efficiency(0.8)), 80);
    }

    #[test]
    fn one_thousand_mb_and_a_hundred_ticks_make_one_fertilizer_for_four_thousand_fe() {
        let mut composter = Composter::from_saved(10_000, 0, 1_000);
        for tick in 1..COMPOSTER_MAX_PROGRESS {
            assert_eq!(
                composter.tick(Augments::NONE, true),
                ComposterStep::Working,
                "tick {tick}"
            );
        }
        assert_eq!(
            composter.tick(Augments::NONE, true),
            ComposterStep::Done {
                mb: 1_000,
                count: 1
            }
        );
        assert_eq!(composter.energy, 6_000, "100 * 40 FE");
        assert_eq!(composter.sewage_mb, 0);
        assert_eq!(composter.progress, 0, "the bar resets after the operation");
    }

    #[test]
    fn the_composter_needs_a_thousand_mb_a_free_slot_and_the_tick_fe() {
        let mut dry = Composter::from_saved(10_000, 0, 999);
        assert_eq!(dry.tick(Augments::NONE, true), ComposterStep::Idle);
        assert_eq!(dry.energy, 10_000, "a gated bar spends nothing");
        let mut full = Composter::from_saved(10_000, 0, 1_000);
        assert_eq!(full.tick(Augments::NONE, false), ComposterStep::Idle);
        assert_eq!(full.progress, 0);
        let mut poor = Composter::from_saved(39, 0, 1_000);
        assert_eq!(poor.tick(Augments::NONE, true), ComposterStep::Idle);
        assert_eq!(poor.energy, 39);
    }

    #[test]
    fn the_composter_bar_length_follows_efficiency() {
        assert_eq!(Composter::bar_length(Augments::NONE), 100);
        assert_eq!(Composter::bar_length(efficiency(0.8)), 80);
        assert_eq!(Composter::bar_length(efficiency(0.0)), 1);
    }

    #[test]
    fn a_speed_augment_on_the_composter_shows_from_the_second_tick() {
        let mut composter = Composter::from_saved(10_000, 0, 1_000);
        composter.tick(speed(4.0), true);
        assert_eq!(composter.progress, 1, "the first tick uses the old rate");
        composter.tick(speed(4.0), true);
        assert_eq!(composter.progress, 5, "then the augment's four a tick");
    }
}
