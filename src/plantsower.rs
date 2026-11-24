//! The Plant Sower's rules, pure and unit-tested (`PlantSowerTile`, `PlantSowerConfig` and
//! `PlantSowerBlock` at `Buuz135/Industrial-Foregoing` `ce2852db`, read for behaviour;
//! `src/runtime.rs` stays untouched).
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile(RangeType.TOP_UP)`: a bar counting **down**
//! from `PlantSowerConfig.maxProgress` **40** (the comment says 100 — stale), a 9-slot input
//! container behind a 9-slot `ItemStackFilter`, and a 10,000 FE buffer at `powerPerOperation`
//! **1000**. At range 0 `TOP_UP` points at the cell **two blocks above** the machine; the soil is
//! the cell one above. On a finished bar, with that cell empty or water and enough FE, the first
//! filtered seed that can be planted there plants the block's default state, shrinks by one and
//! answers `WorkAction(0.2, 1000)` — a `floor(40 × 0.2) = 8`-tick cooldown. A miss with power
//! answers `WorkAction(0.4, 1000 / 50 = 20)` — **20 FE is spent** and the bar rests 16 ticks;
//! only an uncharged buffer answers `WorkAction(1, 0)` with no FE and a full 40-tick bar. Speed
//! and efficiency augments move and shorten the bar as `IndustrialWorkingTile` does.

use crate::runtime::Augments;

/// `PlantSowerConfig.maxProgress` (the field, not the stale "Default: 100" comment).
pub const MAX_PROGRESS: i32 = 40;
/// `PlantSowerConfig.powerPerOperation` (the field and its comment agree here).
pub const POWER_PER_OPERATION: i32 = 1_000;
/// `PlantSowerConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `SidedInventoryComponent("input", 54 + 18 * 3, 22, 9, 0)`.
pub const SLOTS: usize = 9;
/// `ItemStackFilter("filter", 9)`: the nine GUI filter slots (the pin has no filter widget).
pub const FILTER_SLOTS: usize = 9;
/// `floor(40 × 0.2)`: a seed was planted.
pub const COOLDOWN_AFTER_PLANT: i32 = 8;
/// `floor(40 × 0.4)`: a powered attempt planted nothing.
pub const COOLDOWN_MISS_POWERED: i32 = 16;
/// `floor(40 × 1)`: no energy.
pub const COOLDOWN_IDLE: i32 = 40;
/// `WorkAction(0.4, powerPerOperation / 50)` — the powered miss **does** cost FE.
pub const MISS_COST_FE: i32 = POWER_PER_OPERATION / 50;

/// What the game found at the pointed cell on this attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attempt {
    /// The buffer holds less than `POWER_PER_OPERATION` (or the cell cannot be written).
    NoEnergy,
    /// A filtered seed was planted at the pointed cell and one was consumed.
    Planted,
    /// The cell was ready but no seed could be planted (empty input, wrong soil, non-seed).
    Miss,
}

/// What one attempt did to the buffer and the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The bar was still falling; no attempt ran.
    Working,
    /// `WorkAction(1, 0)`: a full rest, no FE.
    Idle { cooldown: i32 },
    /// `WorkAction(0.2, 1000)`: one seed planted, the operation's FE spent.
    Sowed { fe: i32, cooldown: i32 },
    /// `WorkAction(0.4, 20)`: nothing planted, the miss's FE still spent.
    Missed { fe: i32, cooldown: i32 },
}

/// The saved numbers: the FE buffer and the falling bar. The input lives in the block entity's
/// container; the glue passes [`Attempt::Miss`] when no seed can be planted.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantSower {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; a fresh machine starts full and counts down.
    pub progress: i32,
}

impl Default for PlantSower {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
        }
    }
}

impl PlantSower {
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
    /// The cooldown is `floor(bar × workAmount)` — Titanium's `ProgressBarComponent` sets it from
    /// `WorkAction`'s fraction after `onTickWork`.
    pub fn tick(&mut self, augments: Augments, attempt: Attempt) -> Step {
        let bar = Self::bar_length(augments);
        self.progress = (self.progress - augments.speed.max(1)).max(0);
        if self.progress > 0 {
            return Step::Working;
        }
        match attempt {
            // `WorkAction(1, 0)`: the full bar rests and the buffer is untouched.
            Attempt::NoEnergy => {
                self.progress = bar;
                Step::Idle { cooldown: bar }
            }
            // `WorkAction(0.2, powerPerOperation)`: one seed was planted.
            Attempt::Planted => {
                let cooldown = (f64::from(bar) * 0.2).floor() as i32;
                self.energy -= POWER_PER_OPERATION;
                self.progress = cooldown;
                Step::Sowed {
                    fe: POWER_PER_OPERATION,
                    cooldown,
                }
            }
            // `WorkAction(0.4, powerPerOperation / 50)`: the powered miss costs 20 FE.
            Attempt::Miss => {
                let cooldown = (f64::from(bar) * 0.4).floor() as i32;
                self.energy -= MISS_COST_FE;
                self.progress = cooldown;
                Step::Missed {
                    fe: MISS_COST_FE,
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
    fn ready(energy: i32) -> PlantSower {
        PlantSower {
            energy,
            progress: 1,
        }
    }

    #[test]
    fn the_constants_are_the_artefact_own_fields() {
        assert_eq!(MAX_PROGRESS, 40);
        assert_eq!(POWER_PER_OPERATION, 1_000);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(SLOTS, 9);
        assert_eq!(FILTER_SLOTS, 9);
        assert_eq!(COOLDOWN_AFTER_PLANT, 8);
        assert_eq!(COOLDOWN_MISS_POWERED, 16);
        assert_eq!(COOLDOWN_IDLE, 40);
        assert_eq!(MISS_COST_FE, 20);
    }

    #[test]
    fn a_fresh_machine_has_a_full_bar_and_no_power() {
        let sower = PlantSower::default();
        assert_eq!(sower.progress, 40);
        assert_eq!(sower.energy, 0);
    }

    #[test]
    fn saved_values_clamp_to_the_buffer_and_the_bar() {
        let sower = PlantSower::from_saved(90_000, Some(4_000));
        assert_eq!(sower.energy, 10_000);
        assert_eq!(sower.progress, 40);
        let sower = PlantSower::from_saved(-5, None);
        assert_eq!(sower.energy, 0);
        assert_eq!(sower.progress, 40);
    }

    #[test]
    fn the_bar_length_follows_efficiency() {
        assert_eq!(PlantSower::bar_length(Augments::NONE), 40);
        assert_eq!(PlantSower::bar_length(efficiency(0.8)), 32);
        assert_eq!(PlantSower::bar_length(efficiency(0.0)), 1);
    }

    #[test]
    fn a_bar_that_has_not_fallen_only_falls() {
        let mut sower = PlantSower::from_saved(10_000, Some(40));
        assert_eq!(sower.tick(Augments::NONE, Attempt::Planted), Step::Working);
        assert_eq!(sower.progress, 39, "the bar falls one a tick");
        assert_eq!(sower.energy, 10_000, "no attempt, no FE");
        let mut sower = PlantSower::from_saved(10_000, Some(40));
        assert_eq!(sower.tick(speed(4.0), Attempt::Miss), Step::Working);
        assert_eq!(sower.progress, 36);
    }

    #[test]
    fn a_plant_spends_a_thousand_fe_and_rests_eight_ticks() {
        let mut sower = ready(10_000);
        assert_eq!(
            sower.tick(Augments::NONE, Attempt::Planted),
            Step::Sowed {
                fe: 1_000,
                cooldown: 8
            },
            "WorkAction(0.2, 1000): floor(40 * 0.2) = 8"
        );
        assert_eq!(sower.energy, 9_000);
        assert_eq!(sower.progress, 8);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_plant_cooldown_too() {
        let mut sower = ready(10_000);
        assert_eq!(
            sower.tick(efficiency(0.8), Attempt::Planted),
            Step::Sowed {
                fe: 1_000,
                cooldown: 6
            },
            "floor(32 * 0.2) = 6"
        );
        assert_eq!(sower.progress, 6);
    }

    #[test]
    fn a_powered_miss_still_spends_twenty_fe_and_rests_sixteen_ticks() {
        let mut sower = ready(10_000);
        assert_eq!(
            sower.tick(Augments::NONE, Attempt::Miss),
            Step::Missed {
                fe: 20,
                cooldown: 16
            },
            "WorkAction(0.4, powerPerOperation / 50): floor(40 * 0.4) = 16"
        );
        assert_eq!(sower.energy, 9_980);
        assert_eq!(sower.progress, 16);
    }

    #[test]
    fn an_empty_buffer_rests_the_full_bar_and_spends_nothing() {
        let mut sower = ready(999);
        assert_eq!(
            sower.tick(Augments::NONE, Attempt::NoEnergy),
            Step::Idle { cooldown: 40 }
        );
        assert_eq!(sower.energy, 999, "WorkAction(1, 0): nothing is spent");
        assert_eq!(sower.progress, 40);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_faster_and_the_cooldown_uses_the_short_bar() {
        let mut sower = PlantSower::from_saved(10_000, Some(40));
        assert_eq!(sower.tick(speed(4.0), Attempt::Miss), Step::Working);
        assert_eq!(sower.progress, 36);
        let mut sower = ready(10_000);
        assert_eq!(
            sower.tick(speed(4.0), Attempt::Planted),
            Step::Sowed {
                fe: 1_000,
                cooldown: 8
            }
        );
        assert_eq!(sower.energy, 9_000);
    }
}
