// Pure Water Condensator arithmetic for the Industrial Foregoing slice
// (standalone crate lib, injected as `crate::game::industrialforegoing`).
//
// No game access lives here: the progress bar's countdown, the one attempt it
// fires at zero, the water that attempt condenses, the FE it costs, and the two
// Titanium augments the bar reads.
//
// Upstream (`WaterCondensatorTile` and its `IndustrialWorkingTile` base, read
// from Industrial Foregoing `1.21-3.6.39`, and Titanium's
// `ProgressBarComponent`/`AugmentWrapper`, commit `e53e869`) counts the water
// source blocks on the six sides, and once the bar completes: with fewer than
// two sources, nothing; with two or more and at least 1,000 FE in the buffer,
// `sources * 100` mB for 1,000 FE; with two or more and less than that,
// `sources * 50` mB for free. The bar's next value is
// `floor(bar_max * workAmount)`, so the cooldown after an attempt is 10 ticks
// when powered, 50 when not, and 100 when idle on a 100-tick bar.
//
// Augments: a seated speed augment replaces the bar's one-a-tick fall with
// `(int)` of its `Speed` value, and an efficiency augment shortens the bar to
// `floor(100 * efficiency)` after a completed operation, which is what the next
// cooldown is taken from. Both are Titanium's, not this port's invention.

/// `WaterCondensatorConfig.maxStoredPower` default: the FE buffer.
pub const CAPACITY_FE: i32 = 10_000;
/// `WaterCondensatorConfig.maxWaterTankSize` default: the tank.
pub const TANK_CAPACITY_MB: i32 = 16_000;
/// `WaterCondensatorConfig.maxProgress` default: one bar.
pub const MAX_PROGRESS: i32 = 100;
/// `WaterCondensatorConfig.powerPerOperation` default: FE per attempt.
pub const POWER_PER_OPERATION: i32 = 1_000;
/// Water one source block adds on a powered attempt.
pub const WATER_PER_SOURCE_MB: i32 = 100;
/// Water one source block adds on an unpowered attempt.
pub const WATER_PER_SOURCE_UNPOWERED_MB: i32 = 50;
/// `getWaterSources() >= 2`: the machine does nothing below this.
pub const MIN_SOURCES: u32 = 2;
/// Titanium's `AugmentWrapper.ATTACHMENT`: the data component an augment item
/// carries (`titanium:augments`, a `Map<String, Float>`).
pub const AUGMENT_COMPONENT: &str = "titanium:augments";
/// How many augment slots the machine's base tile has
/// (`MachineTile.getAugmentInventory().getSlots()`).
pub const AUGMENT_SLOTS: usize = 2;

/// The yield an augment moves, by Titanium's `AugmentTypes` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AugmentType {
    /// `AugmentTypes.SPEED`: progress per tick.
    Speed,
    /// `AugmentTypes.EFFICIENCY`: the bar's length multiplier.
    Efficiency,
}

impl AugmentType {
    /// Both types this slice reads, speed first as `canAcceptAugment` asks.
    pub const ALL: [Self; 2] = [Self::Speed, Self::Efficiency];

    /// The component's key for this type (`AugmentTypes.*.getType()`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Speed => "Speed",
            Self::Efficiency => "Efficiency",
        }
    }

    /// The type a component key names, if it is one this machine reads.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

/// One augment slot's value: a type and the float the item carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Augment {
    /// Which yield it moves.
    pub kind: AugmentType,
    /// Its `titanium:augments` value.
    pub value: f32,
}

/// The augments in force: what the bar falls by and how long it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Augments {
    /// Progress a tick, `(int)` of the speed value; 1 with no speed augment.
    pub speed: i32,
    /// The bar's length multiplier; 1.0 with no efficiency augment.
    pub efficiency: f32,
}

impl Default for Augments {
    fn default() -> Self {
        Self::NONE
    }
}

impl Augments {
    /// A machine with nothing seated: the base rate every earlier test asserts.
    pub const NONE: Self = Self {
        speed: 1,
        efficiency: 1.0,
    };

    /// The augments of the machine's slots, first of each type winning
    /// (`IndustrialMachineTile.canAcceptAugment` allows one of each).
    #[must_use]
    pub fn from_slots(slots: [Option<Augment>; AUGMENT_SLOTS]) -> Self {
        let mut augments = Self::NONE;
        let mut seen_speed = false;
        let mut seen_efficiency = false;
        for augment in slots.into_iter().flatten() {
            match augment.kind {
                AugmentType::Speed if !seen_speed => {
                    augments.speed = augment.value as i32;
                    seen_speed = true;
                }
                AugmentType::Efficiency if !seen_efficiency => {
                    augments.efficiency = augment.value;
                    seen_efficiency = true;
                }
                _ => {}
            }
        }
        augments
    }

    /// The bar's length, `maxProgress = floor(100 * efficiency)`; never zero.
    #[must_use]
    pub fn bar_max(&self) -> i32 {
        // Java's `(int) Math.floor(getMaxProgress() * efficiency)`: a float
        // multiply, so `0.39999998f` is a 39-tick bar, not 40.
        let scaled = (MAX_PROGRESS as f32 * self.efficiency).floor() as i32;
        scaled.max(1)
    }
}

/// One completed bar's attempt, as `work()` answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    /// Water the attempt would condense (before the tank's capacity).
    pub water_mb: i32,
    /// FE the attempt costs.
    pub energy_fe: i32,
    /// The bar's value after the attempt on a 100-tick bar,
    /// `floor(MAX_PROGRESS * workAmount)`.
    pub cooldown: i32,
    /// The `workAmount` fraction `cooldown` is `floor(100 * …)` of: `(1, 10)`
    /// powered, `(1, 2)` free, `(1, 1)` idle.
    work_amount: (i32, i32),
}

/// What one server tick did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickOutcome {
    /// The bar completed and `work()` ran.
    pub attempted: bool,
    /// Water actually added (the tank's capacity caps it).
    pub water_added_mb: i32,
    /// FE actually spent.
    pub energy_spent_fe: i32,
}

impl Attempt {
    /// `WorkAction` for an attempt with `sources` water sources and `energy`
    /// FE stored.
    #[must_use]
    pub fn new(sources: u32, energy: i32) -> Self {
        if sources < MIN_SOURCES {
            return Self {
                water_mb: 0,
                energy_fe: 0,
                cooldown: MAX_PROGRESS,
                work_amount: (1, 1),
            };
        }
        if energy >= POWER_PER_OPERATION {
            Self {
                water_mb: sources as i32 * WATER_PER_SOURCE_MB,
                energy_fe: POWER_PER_OPERATION,
                cooldown: cooldown(1, 10),
                work_amount: (1, 10),
            }
        } else {
            Self {
                water_mb: sources as i32 * WATER_PER_SOURCE_UNPOWERED_MB,
                energy_fe: 0,
                cooldown: cooldown(1, 2),
                work_amount: (1, 2),
            }
        }
    }

    /// The bar's value after this attempt on a `bar_max`-tick bar,
    /// `floor(bar_max * workAmount)`: the cooldown an efficiency augment
    /// shortens with the bar (Titanium `setProgress(maxProgress * workAmount)`).
    #[must_use]
    pub fn cooldown_in(&self, bar_max: i32) -> i32 {
        bar_max * self.work_amount.0 / self.work_amount.1
    }
}

/// `floor(MAX_PROGRESS * numerator / denominator)`, the bar value upstream
/// sets from `workAmount` (0.1, 0.5 or 1).
#[must_use]
pub fn cooldown(numerator: i32, denominator: i32) -> i32 {
    MAX_PROGRESS * numerator / denominator
}
/// The machine's saved state: the `energy` int (`native_block_entity_energy`
/// keeps it there), the tank and the bar's progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condensator {
    /// FE in the buffer.
    pub energy: i32,
    /// Water in the tank, millibuckets.
    pub water_mb: i32,
    /// The bar, `1..=MAX_PROGRESS`; 0 is a completed bar waiting for its tick.
    pub progress: i32,
}

impl Default for Condensator {
    fn default() -> Self {
        Self {
            energy: 0,
            water_mb: 0,
            progress: MAX_PROGRESS,
        }
    }
}

impl Condensator {
    /// A machine from its saved compound; a missing or out-of-range value is
    /// clamped, and a missing bar starts full as a new block's does.
    #[must_use]
    pub fn from_saved(energy: i32, water_mb: i32, progress: Option<i32>) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            water_mb: water_mb.clamp(0, TANK_CAPACITY_MB),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
        }
    }

    /// Room left in the FE buffer.
    #[must_use]
    pub fn energy_room(&self) -> i32 {
        (CAPACITY_FE - self.energy).max(0)
    }

    /// Insert FE into the buffer, answering how much was taken (the buffer's
    /// `SimpleEnergyHandler` accepts up to its capacity).
    pub fn insert_energy(&mut self, amount: i32) -> i32 {
        let taken = amount.max(0).min(self.energy_room());
        self.energy += taken;
        taken
    }

    /// Room left in the tank.
    #[must_use]
    pub fn tank_room(&self) -> i32 {
        (TANK_CAPACITY_MB - self.water_mb).max(0)
    }

    /// One server tick at the base rate: one progress a tick, the same attempt.
    pub fn tick(&mut self, sources: u32) -> TickOutcome {
        self.tick_with_augments(sources, Augments::NONE)
    }

    /// One server tick with the seated augments: the bar falls by the speed
    /// augment and, at zero, an attempt runs; the bar then rests at
    /// `floor(bar_max * workAmount)`, where `bar_max` is the efficiency-scaled
    /// length Titanium sets after a completed operation.
    pub fn tick_with_augments(&mut self, sources: u32, augments: Augments) -> TickOutcome {
        if self.progress > 0 {
            // Titanium's `setProgress(progress - progressIncrease)`: the bar
            // falls by the speed augment, and may reach or pass zero.
            self.progress -= augments.speed.max(0);
        }
        if self.progress > 0 {
            return TickOutcome::default();
        }
        let attempt = Attempt::new(sources, self.energy);
        let added = attempt.water_mb.min(self.tank_room()).max(0);
        self.water_mb += added;
        let spent = attempt.energy_fe.min(self.energy).max(0);
        self.energy -= spent;
        self.progress = attempt.cooldown_in(augments.bar_max());
        TickOutcome {
            attempted: true,
            water_added_mb: added,
            energy_spent_fe: spent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A new machine, as `block_entity` makes one.
    fn fresh() -> Condensator {
        Condensator::default()
    }

    #[test]
    fn a_new_machine_is_empty_and_its_bar_is_full() {
        let machine = fresh();
        assert_eq!(machine.energy, 0);
        assert_eq!(machine.water_mb, 0);
        assert_eq!(machine.progress, MAX_PROGRESS);
    }

    #[test]
    fn the_first_attempt_is_after_one_hundred_ticks() {
        let mut machine = fresh();
        machine.energy = CAPACITY_FE;
        for _ in 0..MAX_PROGRESS - 1 {
            let outcome = machine.tick(MIN_SOURCES);
            assert!(!outcome.attempted);
        }
        assert_eq!(machine.progress, 1);
        assert_eq!(machine.water_mb, 0);
        let outcome = machine.tick(MIN_SOURCES);
        assert!(outcome.attempted);
        assert_eq!(outcome.water_added_mb, 200);
    }

    #[test]
    fn a_powered_attempt_condenses_one_hundred_a_source() {
        let mut machine = Condensator {
            energy: POWER_PER_OPERATION,
            ..Condensator {
                progress: 1,
                ..fresh()
            }
        };
        let outcome = machine.tick(2);
        assert_eq!(outcome.water_added_mb, 200);
        assert_eq!(outcome.energy_spent_fe, POWER_PER_OPERATION);
        assert_eq!(machine.water_mb, 200);
        assert_eq!(machine.energy, 0);
    }

    #[test]
    fn one_source_condenses_nothing() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        let outcome = machine.tick(1);
        assert!(outcome.attempted);
        assert_eq!(outcome.water_added_mb, 0);
        assert_eq!(outcome.energy_spent_fe, 0);
        assert_eq!(machine.water_mb, 0);
        assert_eq!(machine.energy, CAPACITY_FE);
    }

    #[test]
    fn no_sources_condense_nothing() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        let outcome = machine.tick(0);
        assert_eq!(outcome.water_added_mb, 0);
        assert_eq!(outcome.energy_spent_fe, 0);
    }

    #[test]
    fn an_unpowered_attempt_condenses_half_for_free() {
        let mut machine = Condensator {
            energy: POWER_PER_OPERATION - 1,
            progress: 1,
            ..fresh()
        };
        let outcome = machine.tick(2);
        assert_eq!(outcome.water_added_mb, 100);
        assert_eq!(outcome.energy_spent_fe, 0);
        assert_eq!(machine.energy, POWER_PER_OPERATION - 1);
    }

    #[test]
    fn six_sources_condense_six_hundred() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        assert_eq!(machine.tick(6).water_added_mb, 600);
        assert_eq!(machine.water_mb, 600);
        assert_eq!(machine.energy, CAPACITY_FE - POWER_PER_OPERATION);
    }

    #[test]
    fn an_unpowered_attempt_with_four_sources_condenses_two_hundred() {
        let mut machine = Condensator {
            progress: 1,
            ..fresh()
        };
        assert_eq!(machine.tick(4).water_added_mb, 200);
    }

    #[test]
    fn the_cooldown_is_ten_powered_fifty_unpowered_and_a_hundred_idle() {
        assert_eq!(Attempt::new(2, CAPACITY_FE).cooldown, 10);
        assert_eq!(Attempt::new(2, 0).cooldown, 50);
        assert_eq!(Attempt::new(1, CAPACITY_FE).cooldown, 100);
        assert_eq!(Attempt::new(0, 0).cooldown, 100);
    }

    #[test]
    fn a_powered_operation_is_followed_by_another_one_ten_ticks_later() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        machine.tick(2);
        for _ in 0..9 {
            assert!(!machine.tick(2).attempted);
        }
        let outcome = machine.tick(2);
        assert!(outcome.attempted);
        assert_eq!(outcome.water_added_mb, 200);
    }

    #[test]
    fn an_idle_bar_waits_a_hundred_ticks() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        machine.tick(0);
        assert_eq!(machine.progress, MAX_PROGRESS);
        for _ in 0..MAX_PROGRESS - 1 {
            assert!(!machine.tick(0).attempted);
        }
        assert!(machine.tick(0).attempted);
    }

    #[test]
    fn an_unpowered_bar_waits_fifty_ticks() {
        let mut machine = Condensator {
            progress: 1,
            ..fresh()
        };
        machine.tick(2);
        assert_eq!(machine.progress, 50);
        for _ in 0..49 {
            assert!(!machine.tick(2).attempted);
        }
        assert!(machine.tick(2).attempted);
    }

    #[test]
    fn the_tank_stops_at_its_capacity_and_the_fe_is_still_spent() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            water_mb: TANK_CAPACITY_MB - 100,
            progress: 1,
        };
        let outcome = machine.tick(2);
        assert_eq!(outcome.water_added_mb, 100);
        assert_eq!(outcome.energy_spent_fe, POWER_PER_OPERATION);
        assert_eq!(machine.water_mb, TANK_CAPACITY_MB);
        assert_eq!(machine.energy, CAPACITY_FE - POWER_PER_OPERATION);
    }

    #[test]
    fn a_full_tank_still_spends_the_fe() {
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            water_mb: TANK_CAPACITY_MB,
            progress: 1,
        };
        let outcome = machine.tick(6);
        assert_eq!(outcome.water_added_mb, 0);
        assert_eq!(outcome.energy_spent_fe, POWER_PER_OPERATION);
        assert_eq!(machine.water_mb, TANK_CAPACITY_MB);
    }

    #[test]
    fn inserting_fe_stops_at_the_capacity_and_refuses_negatives() {
        let mut machine = fresh();
        assert_eq!(machine.insert_energy(-5), 0);
        assert_eq!(machine.insert_energy(4_000), 4_000);
        assert_eq!(machine.insert_energy(CAPACITY_FE), CAPACITY_FE - 4_000);
        assert_eq!(machine.energy, CAPACITY_FE);
        assert_eq!(machine.insert_energy(1), 0);
    }

    #[test]
    fn the_room_is_never_negative() {
        let machine = Condensator {
            energy: CAPACITY_FE + 1,
            water_mb: TANK_CAPACITY_MB + 1,
            progress: 0,
        };
        assert_eq!(machine.energy_room(), 0);
        assert_eq!(machine.tank_room(), 0);
    }

    #[test]
    fn from_saved_clamps_and_defaults_a_missing_bar_to_full() {
        let machine = Condensator::from_saved(-4, 999_999, None);
        assert_eq!(machine.energy, 0);
        assert_eq!(machine.water_mb, TANK_CAPACITY_MB);
        assert_eq!(machine.progress, MAX_PROGRESS);
        let machine = Condensator::from_saved(12, 7, Some(250));
        assert_eq!(machine.energy, 12);
        assert_eq!(machine.water_mb, 7);
        assert_eq!(machine.progress, MAX_PROGRESS);
    }

    #[test]
    fn the_last_fe_buys_one_powered_attempt() {
        let mut machine = Condensator {
            energy: POWER_PER_OPERATION,
            progress: 1,
            ..fresh()
        };
        assert_eq!(machine.tick(2).energy_spent_fe, POWER_PER_OPERATION);
        machine.progress = 1;
        let outcome = machine.tick(2);
        assert_eq!(outcome.energy_spent_fe, 0);
        assert_eq!(outcome.water_added_mb, 100);
    }

    // -----------------------------------------------------------------------
    // The augments (`titanium:augments`, Titanium's `AugmentTypes`)
    // -----------------------------------------------------------------------

    /// A speed augment of `value`, as an item's component decodes to one.
    fn speed(value: f32) -> Augment {
        Augment {
            kind: AugmentType::Speed,
            value,
        }
    }

    /// An efficiency augment of `value`.
    fn efficiency(value: f32) -> Augment {
        Augment {
            kind: AugmentType::Efficiency,
            value,
        }
    }

    /// A machine with a full buffer and a full bar, ready to run.
    fn ready() -> Condensator {
        Condensator {
            energy: CAPACITY_FE,
            ..fresh()
        }
    }

    #[test]
    fn a_speed_augment_falls_four_a_tick() {
        let augments = Augments::from_slots([Some(speed(4.0)), None]);
        let mut machine = ready();
        for _ in 0..10 {
            machine.tick_with_augments(MIN_SOURCES, augments);
        }
        assert_eq!(machine.progress, 60, "10 ticks at 4 a tick from 100");
    }

    #[test]
    fn the_same_machine_without_an_augment_falls_one_a_tick() {
        let mut machine = ready();
        for _ in 0..10 {
            machine.tick(MIN_SOURCES);
        }
        assert_eq!(machine.progress, 90, "10 ticks at 1 a tick from 100");
    }

    #[test]
    fn a_speed_augment_finishes_the_bar_in_twenty_five_ticks() {
        let augments = Augments::from_slots([Some(speed(4.0)), None]);
        let mut machine = ready();
        for _ in 0..24 {
            assert!(!machine.tick_with_augments(MIN_SOURCES, augments).attempted);
        }
        assert_eq!(machine.progress, 4);
        let outcome = machine.tick_with_augments(MIN_SOURCES, augments);
        assert!(outcome.attempted, "the 25th tick completes a 100 bar at 4");
        assert_eq!(outcome.water_added_mb, 200);
        assert_eq!(machine.progress, 10, "the powered cooldown is unchanged");
    }

    /// Titanium: `(int) AugmentWrapper.getType(speed)` truncates toward zero.
    #[test]
    fn the_speed_value_truncates_toward_zero_like_a_java_cast() {
        assert_eq!(
            Augments::from_slots([Some(speed(4.9)), None]).speed,
            4,
            "(int) 4.9f is 4"
        );
        assert_eq!(
            Augments::from_slots([Some(speed(2.0)), None]).speed,
            2,
            "the parent's tier 1 is 1 + 1"
        );
        assert_eq!(
            Augments::from_slots([Some(speed(0.5)), None]).speed,
            0,
            "(int) 0.5f is 0, as the Java cast is"
        );
    }

    /// `maxProgress = floor(100 * efficiency)`, Titanium's `setMaxProgress`
    /// after a completed operation, so the next cooldown is scaled too.
    #[test]
    fn an_efficiency_augment_changes_the_bar_maximum_after_a_work() {
        let augments = Augments::from_slots([None, Some(efficiency(0.8))]);
        assert_eq!(augments.bar_max(), 80);
        let mut machine = Condensator {
            energy: CAPACITY_FE,
            progress: 1,
            ..fresh()
        };
        assert_eq!(
            machine
                .tick_with_augments(MIN_SOURCES, augments)
                .water_added_mb,
            200
        );
        assert_eq!(
            machine.progress, 8,
            "floor(80 * 0.1) is the powered cooldown on the scaled bar"
        );
        machine.tick_with_augments(MIN_SOURCES, augments);
        assert_eq!(
            machine.progress, 7,
            "the scaled bar keeps falling one a tick"
        );
    }

    /// Java's `(int) Math.floor(100 * float)` on the values the ecosystem
    /// declares: 0.9 → 90, 0.8 → 80, 0.7 → 70, 0.5 → 50, and mifa's tier 6
    /// `0.39999998f` → 39 (not 40).
    #[test]
    fn the_efficiency_multiplier_floors_like_java_float_math() {
        let max = |value| Augments::from_slots([None, Some(efficiency(value))]).bar_max();
        assert_eq!(max(1.0), 100);
        assert_eq!(max(0.9), 90);
        assert_eq!(max(0.8), 80);
        assert_eq!(max(0.7), 70);
        assert_eq!(max(0.5), 50);
        assert_eq!(max(0.399_999_98), 39);
        assert_eq!(max(0.0), 1, "a bar never has a zero maximum");
    }

    /// `IndustrialMachineTile.canAcceptAugment`: one of each type; the first
    /// slot with a type is the one that counts.
    #[test]
    fn one_of_each_augment_type_is_read_and_a_duplicate_is_ignored() {
        let base = Augments::NONE;
        assert_eq!(base.speed, 1);
        assert_eq!(base.efficiency, 1.0);
        assert_eq!(Augments::from_slots([None, None]), base);
        let both = Augments::from_slots([Some(speed(3.0)), Some(efficiency(0.8))]);
        assert_eq!(both.speed, 3);
        assert_eq!(both.efficiency, 0.8);
        let two_speeds = Augments::from_slots([Some(speed(3.0)), Some(speed(9.0))]);
        assert_eq!(two_speeds.speed, 3, "the first slot wins");
        let second = Augments::from_slots([None, Some(speed(2.0))]);
        assert_eq!(second.speed, 2);
    }

    #[test]
    fn an_augment_type_round_trips_through_its_component_name() {
        for kind in AugmentType::ALL {
            assert_eq!(AugmentType::from_name(kind.name()), Some(kind));
        }
        assert_eq!(AugmentType::from_name("Speed"), Some(AugmentType::Speed));
        assert_eq!(
            AugmentType::from_name("Efficiency"),
            Some(AugmentType::Efficiency)
        );
        assert_eq!(AugmentType::from_name("Processing"), None);
        assert_eq!(AugmentType::from_name("speed"), None);
    }

    /// The base-rate path is what every slice-1 and slice-2 test asserts, so
    /// the augment path must be exactly it when no augment is seated.
    #[test]
    fn the_augment_path_with_no_augment_is_the_base_rate() {
        let mut plain = ready();
        let mut augmented = ready();
        for _ in 0..MAX_PROGRESS {
            assert_eq!(
                plain.tick(MIN_SOURCES),
                augmented.tick_with_augments(MIN_SOURCES, Augments::NONE)
            );
        }
        assert_eq!(plain, augmented);
    }
}
