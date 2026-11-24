//! The Plant Gatherer's rules, pure and unit-tested (`PlantGathererTile` and
//! `PlantGathererConfig` at `Buuz135/Industrial-Foregoing` `ce2852db`, read for behaviour;
//! `src/runtime.rs` stays untouched).
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile(RangeType.BEHIND)`: a bar counting **down**
//! from `PlantGathererConfig.maxProgress` **40** (the comment says 100 — stale), a 15-slot
//! output-only container, a 10,000 FE buffer and `powerPerOperation` **400**. At range 0 the
//! working area is the single cell behind the machine's `subfacing` (`amount = max(1, size/20)
//! = 1`). A mature crop there is harvested by the matching `PlantRecollectable`: its **actual**
//! block drops go into the output, the cell becomes air, and the work answers
//! `WorkAction(0.3, 400)` — a `floor(40 × 0.3) = 12`-tick cooldown. A block that cannot be
//! harvested, a full output or an undercharged buffer answers `WorkAction(1, 0)`: a full bar and
//! no FE. Speed and efficiency augments move the bar and shorten it as in `IndustrialWorkingTile`.

use crate::runtime::Augments;

/// `PlantGathererConfig.maxProgress` (the field, not the stale "Default: 100" comment).
pub const MAX_PROGRESS: i32 = 40;
/// `PlantGathererConfig.powerPerOperation`.
pub const POWER_PER_OPERATION: i32 = 400;
/// `PlantGathererConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `SidedInventoryComponent("output", 78, 22, 3 * 5, 0)`.
pub const OUTPUT_SLOTS: usize = 15;
/// `floor(40 × 0.3)`: a crop was harvested.
pub const COOLDOWN_AFTER_HARVEST: i32 = 12;
/// `floor(40 × 1)`: nothing was harvested.
pub const COOLDOWN_IDLE: i32 = 40;
/// The ether bar's own maximum (`ProgressBarComponent(…, 0, 100)`); outside this slice's scope.
pub const ETHER_BAR_MAX: i32 = 100;

/// What the game found at the pointed block on this attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// The buffer holds less than `POWER_PER_OPERATION`.
    NoEnergy,
    /// No recollectable matched the pointed block (an immature crop, air, stone…).
    NotATarget,
    /// The 15 output slots have nowhere to put another drop.
    OutputFull,
    /// A mature crop was harvested; `stacks` is the number of drop stacks inserted.
    Harvested { stacks: usize },
}

/// What one attempt did to the buffer and the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The bar was still falling; no attempt ran.
    Working,
    /// Nothing was gathered: a full bar of rest and no FE.
    Idle { cooldown: i32 },
    /// One operation: `fe` spent, `stacks` inserted and a `cooldown`-tick rest.
    Gathered {
        fe: i32,
        cooldown: i32,
        stacks: usize,
    },
}

/// The saved numbers: the FE buffer and the falling bar. The output lives in the block entity's
/// container; the glue passes [`Attempt::OutputFull`] when every slot is occupied.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantGatherer {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; a fresh machine starts full and counts down.
    pub progress: i32,
}

impl Default for PlantGatherer {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
        }
    }
}

impl PlantGatherer {
    /// A machine from its save: the buffer clamps and a bar the save never wrote starts full.
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
        }
    }

    /// The bar's length with the seated augments: `floor(40 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (f64::from(MAX_PROGRESS) * f64::from(augments.efficiency))
            .floor()
            .max(1.0) as i32
    }

    /// One server tick: fall the bar by the speed-augmented step and, at zero, run the attempt.
    /// The cooldown is `floor(bar × 0.3)` after a harvest, `floor(bar × 1)` after a miss — Titanium's
    /// `ProgressBarComponent` sets `progress = floor(maxProgress × workAmount)` after `onTickWork`.
    pub fn tick(&mut self, augments: Augments, attempt: Attempt) -> Step {
        let bar = Self::bar_length(augments);
        self.progress = (self.progress - augments.speed.max(1)).max(0);
        if self.progress > 0 {
            return Step::Working;
        }
        match attempt {
            Attempt::NoEnergy | Attempt::NotATarget | Attempt::OutputFull => {
                // `WorkAction(1, 0)`: a full bar of rest and no FE.
                self.progress = bar;
                Step::Idle { cooldown: bar }
            }
            Attempt::Harvested { stacks } => {
                // The glue passes `Harvested` only after the `hasEnergy(400)` gate, so the whole
                // cost is payable. `WorkAction(0.3, 400)`.
                let cooldown = (f64::from(bar) * 0.3).floor() as i32;
                self.energy -= POWER_PER_OPERATION;
                self.progress = cooldown;
                Step::Gathered {
                    fe: POWER_PER_OPERATION,
                    cooldown,
                    stacks,
                }
            }
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

    /// A machine one tick from its attempt.
    fn ready(energy: i32) -> PlantGatherer {
        PlantGatherer {
            energy,
            progress: 1,
        }
    }

    #[test]
    fn the_constants_are_the_artefact_own_fields() {
        assert_eq!(MAX_PROGRESS, 40);
        assert_eq!(POWER_PER_OPERATION, 400);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(OUTPUT_SLOTS, 15);
        assert_eq!(COOLDOWN_AFTER_HARVEST, 12);
        assert_eq!(COOLDOWN_IDLE, 40);
        assert_eq!(ETHER_BAR_MAX, 100);
    }

    #[test]
    fn a_fresh_machine_has_a_full_bar_and_no_power() {
        let gatherer = PlantGatherer::default();
        assert_eq!(gatherer.progress, 40);
        assert_eq!(gatherer.energy, 0);
    }

    #[test]
    fn saved_values_clamp_to_the_buffer_and_the_bar() {
        let gatherer = PlantGatherer::from_saved(90_000, Some(4_000));
        assert_eq!(gatherer.energy, 10_000);
        assert_eq!(gatherer.progress, 40);
        let gatherer = PlantGatherer::from_saved(-5, None);
        assert_eq!(gatherer.energy, 0);
        assert_eq!(gatherer.progress, 40);
    }

    #[test]
    fn the_bar_length_follows_efficiency() {
        assert_eq!(PlantGatherer::bar_length(Augments::NONE), 40);
        assert_eq!(PlantGatherer::bar_length(efficiency(0.8)), 32);
        assert_eq!(PlantGatherer::bar_length(efficiency(0.0)), 1);
    }

    #[test]
    fn a_bar_that_has_not_fallen_only_falls() {
        let mut gatherer = PlantGatherer::from_saved(10_000, Some(40));
        assert_eq!(
            gatherer.tick(Augments::NONE, Attempt::Harvested { stacks: 2 }),
            Step::Working
        );
        assert_eq!(gatherer.progress, 39, "the bar falls one a tick");
        assert_eq!(gatherer.energy, 10_000, "no attempt, no FE");
        let mut gatherer = PlantGatherer::from_saved(10_000, Some(40));
        assert_eq!(gatherer.tick(speed(4.0), Attempt::NoEnergy), Step::Working);
        assert_eq!(gatherer.progress, 36);
    }

    #[test]
    fn a_harvest_spends_four_hundred_fe_and_rests_twelve_ticks() {
        let mut gatherer = ready(10_000);
        assert_eq!(
            gatherer.tick(Augments::NONE, Attempt::Harvested { stacks: 2 }),
            Step::Gathered {
                fe: 400,
                cooldown: 12,
                stacks: 2
            },
            "WorkAction(0.3, 400): floor(40 * 0.3) = 12"
        );
        assert_eq!(gatherer.energy, 9_600);
        assert_eq!(gatherer.progress, 12);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_harvest_cooldown_too() {
        let mut gatherer = ready(10_000);
        assert_eq!(
            gatherer.tick(efficiency(0.8), Attempt::Harvested { stacks: 1 }),
            Step::Gathered {
                fe: 400,
                cooldown: 9,
                stacks: 1
            },
            "floor(32 * 0.3) = 9"
        );
        assert_eq!(gatherer.progress, 9);
    }

    #[test]
    fn an_undercharged_buffer_gathers_nothing_and_rests_the_full_bar() {
        let mut gatherer = ready(399);
        assert_eq!(
            gatherer.tick(Augments::NONE, Attempt::NoEnergy),
            Step::Idle { cooldown: 40 }
        );
        assert_eq!(
            gatherer.energy, 399,
            "hasEnergy(400) failed: nothing is spent"
        );
        assert_eq!(gatherer.progress, 40);
    }

    #[test]
    fn an_immature_or_absent_crop_rests_the_full_bar() {
        let mut gatherer = ready(10_000);
        assert_eq!(
            gatherer.tick(Augments::NONE, Attempt::NotATarget),
            Step::Idle { cooldown: 40 }
        );
        assert_eq!(gatherer.energy, 10_000);
        assert_eq!(gatherer.progress, 40);
    }

    #[test]
    fn a_full_output_rests_the_full_bar_without_spending() {
        let mut gatherer = ready(10_000);
        assert_eq!(
            gatherer.tick(Augments::NONE, Attempt::OutputFull),
            Step::Idle { cooldown: 40 }
        );
        assert_eq!(gatherer.energy, 10_000);
        assert_eq!(gatherer.progress, 40);
    }
}
