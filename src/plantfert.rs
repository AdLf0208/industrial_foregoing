//! The Plant Fertilizer's rules, pure and unit-tested (`PlantFertilizerTile` at
//! `Buuz135/Industrial-Foregoing` `ce2852db`, read for behaviour; `src/runtime.rs` stays untouched).
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile(RangeType.BEHIND)`: a bar counting **down** from
//! `PlantFertilizerConfig.maxProgress` **50**, an 18-slot input container accepting only
//! `c:fertilizers`, and a 10,000 FE buffer. On a finished bar, with a fertilizer in hand and at
//! least `powerPerOperation` **1000** FE, it bonemeals the single block behind the machine: it
//! shrinks the stack by one, calls `performBonemeal`, and rests the bar at `floor(50 × 0.25) = 12`
//! ticks if the crop is still a valid target, or `floor(50 × 0.5) = 25` and advances the pointer if
//! it is finished. A block that is not a valid target answers `WorkAction(0.5, 0)`; no input or no
//! energy answers `WorkAction(1, 0)` — a full bar and no FE spent. The config's *comments* say 100
//! and 400 FE, but its **fields** are 50 and 1000; the fields are the artefact's behaviour.

use crate::runtime::Augments;

/// `PlantFertilizerConfig.maxProgress` (the field, not the stale "Default: 100" comment).
pub const MAX_PROGRESS: i32 = 50;
/// `PlantFertilizerConfig.powerPerOperation` (the field, not the stale "400FE" comment).
pub const POWER_PER_OPERATION: i32 = 1000;
/// `PlantFertilizerConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `SidedInventoryComponent("fertilizer", 50, 22, 3 * 6, 0)`.
pub const SLOTS: usize = 18;
/// `floor(50 × 0.25)`: the crop grew and can still grow again.
pub const COOLDOWN_AFTER_GROW: i32 = 12;
/// `floor(50 × 0.5)`: the crop is finished, or was never a valid target.
pub const COOLDOWN_AFTER_FINISH: i32 = 25;
/// `floor(50 × 1)`: no input, no energy.
pub const COOLDOWN_IDLE: i32 = 50;
/// The input the 18 slots accept (`c:fertilizers` includes this and bone meal).
pub const FERTILIZER: &str = "industrialforegoing:fertilizer";

/// What the game found at the pointed block on this attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// No fertilizer in any of the 18 slots.
    NoInput,
    /// Fertilizer present but the buffer holds less than `POWER_PER_OPERATION`.
    NoEnergy,
    /// Not a bonemealable block, or the success roll refused it.
    NotATarget,
    /// Bonemeal grew the block; `still_valid` says whether it remains a valid target afterwards.
    Grew { still_valid: bool },
}

/// What one tick did to the buffer, the input and the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The bar was still falling; no attempt ran.
    Working,
    /// No operation: `cooldown` ticks of rest and no FE.
    Idle { cooldown: i32 },
    /// One fertilizer was consumed and `fe` spent; the bar rests for `cooldown` ticks.
    Fertilized {
        consumed: i32,
        fe: i32,
        cooldown: i32,
    },
}

/// The saved numbers: the buffer and the falling bar. The input lives in the block entity's
/// container, so the glue reads it and passes [`Attempt::NoInput`] when all 18 slots are empty.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantFertilizer {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; a fresh machine starts full and counts down.
    pub progress: i32,
}

impl Default for PlantFertilizer {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
        }
    }
}

impl PlantFertilizer {
    /// A machine from its save: values clamp to the buffer and a bar the save never wrote starts
    /// full (`ProgressBarComponent(50, 50)`).
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
        }
    }

    /// The bar's length with the seated augments: `floor(50 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        (f64::from(MAX_PROGRESS) * f64::from(augments.efficiency))
            .floor()
            .max(1.0) as i32
    }

    /// One server tick: fall the bar by the speed-augmented step, and at zero run the attempt.
    /// `clamp`ped cooldowns are `floor(bar × workAmount)`, where `bar` is the augmented length.
    pub fn tick(&mut self, augments: Augments, attempt: Attempt) -> Step {
        let bar = Self::bar_length(augments);
        self.progress = (self.progress - augments.speed.max(1)).max(0);
        if self.progress > 0 {
            return Step::Working;
        }
        match attempt {
            Attempt::NoInput | Attempt::NoEnergy => {
                // `WorkAction(1, 0)`: the full bar rests and nothing is spent.
                self.progress = bar;
                Step::Idle { cooldown: bar }
            }
            Attempt::NotATarget => {
                // `WorkAction(0.5, 0)` after `increasePointer()`: the block was not a target.
                self.progress = (f64::from(bar) * 0.5).floor() as i32;
                Step::Idle {
                    cooldown: self.progress,
                }
            }
            Attempt::Grew { still_valid } => {
                // The glue passes `Grew` only after the `hasEnergy(1000)` gate, so the full cost is
                // payable. `WorkAction(0.25|0.5, 1000)` rests `floor(bar × amount)` ticks.
                let cooldown = if still_valid {
                    (f64::from(bar) * 0.25).floor() as i32
                } else {
                    (f64::from(bar) * 0.5).floor() as i32
                };
                self.energy -= POWER_PER_OPERATION;
                self.progress = cooldown;
                Step::Fertilized {
                    consumed: 1,
                    fe: POWER_PER_OPERATION,
                    cooldown,
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
    fn ready(energy: i32) -> PlantFertilizer {
        PlantFertilizer {
            energy,
            progress: 1,
        }
    }

    #[test]
    fn the_constants_are_the_artefact_own_fields() {
        assert_eq!(MAX_PROGRESS, 50);
        assert_eq!(POWER_PER_OPERATION, 1000);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(SLOTS, 18);
        assert_eq!(COOLDOWN_AFTER_GROW, 12);
        assert_eq!(COOLDOWN_AFTER_FINISH, 25);
        assert_eq!(COOLDOWN_IDLE, 50);
        assert_eq!(FERTILIZER, "industrialforegoing:fertilizer");
    }

    #[test]
    fn a_fresh_machine_has_a_full_bar_and_no_power() {
        let plant = PlantFertilizer::default();
        assert_eq!(plant.progress, 50);
        assert_eq!(plant.energy, 0);
    }

    #[test]
    fn saved_values_clamp_to_the_buffer_and_the_bar() {
        let plant = PlantFertilizer::from_saved(90_000, Some(4_000));
        assert_eq!(plant.energy, 10_000);
        assert_eq!(plant.progress, 50);
        let plant = PlantFertilizer::from_saved(-5, None);
        assert_eq!(plant.energy, 0);
        assert_eq!(plant.progress, 50);
    }

    #[test]
    fn the_bar_length_follows_efficiency() {
        assert_eq!(PlantFertilizer::bar_length(Augments::NONE), 50);
        assert_eq!(PlantFertilizer::bar_length(efficiency(0.8)), 40);
        assert_eq!(PlantFertilizer::bar_length(efficiency(0.0)), 1);
    }

    #[test]
    fn a_bar_that_has_not_fallen_only_falls() {
        let mut plant = PlantFertilizer::from_saved(10_000, Some(50));
        assert_eq!(plant.tick(Augments::NONE, Attempt::Grew { still_valid: true }), Step::Working);
        assert_eq!(plant.progress, 49, "the bar falls one a tick");
        assert_eq!(plant.energy, 10_000, "no attempt, no FE");
        let mut plant = PlantFertilizer::from_saved(10_000, Some(50));
        assert_eq!(plant.tick(speed(4.0), Attempt::NoInput), Step::Working);
        assert_eq!(plant.progress, 46);
    }

    #[test]
    fn growing_a_crop_that_can_grow_again_costs_one_and_spends_one_thousand() {
        let mut plant = ready(10_000);
        assert_eq!(
            plant.tick(Augments::NONE, Attempt::Grew { still_valid: true }),
            Step::Fertilized {
                consumed: 1,
                fe: 1000,
                cooldown: 12
            },
            "WorkAction(0.25, 1000): floor(50 * 0.25) = 12"
        );
        assert_eq!(plant.energy, 9_000);
        assert_eq!(plant.progress, 12);
    }

    #[test]
    fn growing_a_finished_crop_costs_one_and_rests_twenty_five_ticks() {
        let mut plant = ready(10_000);
        assert_eq!(
            plant.tick(Augments::NONE, Attempt::Grew { still_valid: false }),
            Step::Fertilized {
                consumed: 1,
                fe: 1000,
                cooldown: 25
            },
            "WorkAction(0.5, 1000) after increasePointer"
        );
        assert_eq!(plant.energy, 9_000);
        assert_eq!(plant.progress, 25);
    }

    #[test]
    fn a_block_that_is_not_a_target_consumes_nothing_and_spends_nothing() {
        let mut plant = ready(10_000);
        assert_eq!(
            plant.tick(Augments::NONE, Attempt::NotATarget),
            Step::Idle { cooldown: 25 },
            "WorkAction(0.5, 0)"
        );
        assert_eq!(plant.energy, 10_000);
        assert_eq!(plant.progress, 25);
    }

    #[test]
    fn no_fertilizer_rests_the_full_bar() {
        let mut plant = ready(10_000);
        assert_eq!(plant.tick(Augments::NONE, Attempt::NoInput), Step::Idle { cooldown: 50 });
        assert_eq!(plant.energy, 10_000, "no FE spent without an input");
        assert_eq!(plant.progress, 50);
    }

    #[test]
    fn an_undercharged_buffer_grows_nothing_and_rests_the_full_bar() {
        let mut plant = ready(999);
        assert_eq!(plant.tick(Augments::NONE, Attempt::NoEnergy), Step::Idle { cooldown: 50 });
        assert_eq!(plant.energy, 999, "hasEnergy(1000) failed: nothing is spent");
        assert_eq!(plant.progress, 50);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_cooldown_too() {
        let mut plant = ready(10_000);
        assert_eq!(
            plant.tick(efficiency(0.8), Attempt::Grew { still_valid: true }),
            Step::Fertilized {
                consumed: 1,
                fe: 1000,
                cooldown: 10
            },
            "floor(40 * 0.25) = 10"
        );
        assert_eq!(plant.progress, 10);
    }
}
