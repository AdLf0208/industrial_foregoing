//! The Mob Duplicator's rules (`MobDuplicatorTile`), pure and unit-tested.
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile(TOP_UP)`: a bar that counts **down** from
//! `maxProgress` **62**, and at zero one attempt in a **1×3 column two blocks above** the machine.
//! The attempt needs a **Mob Imprisonment Tool** in its one input slot and **5,000 FE**
//! (`MobDuplicatorConfig.powerPerOperation`), a non-empty **8,000 mB** essence tank, a readable
//! entity that is not in `industrialforegoing:mob_duplicator_blacklist`, and **32 or fewer** mobs
//! of that class already in the working area.
//!
//! The cost is `(int)(entity.getHealth() * 12)` mB (`essenceNeeded`); `canSpawn` is the tank
//! divided by it, and one operation spawns `1 + random.nextInt(min(canSpawn, 4))` fresh mobs,
//! draining the essence each one costs. A successful attempt spends **5,000 FE** and rests the bar
//! **full** (`WorkAction(1f, powerPerOperation)`); a refused one answers `WorkAction(1, 0)` — no
//! FE, full bar.
//!
//! Everything the game supplies — the FE, the bar, the tank, the captured health, the nearby count
//! and the random draw — is this module's input; the glue (`crate::mobduplicator`) reads and writes
//! it.

use crate::runtime::{AUGMENT_SLOTS, Augment, Augments};

/// `MobDuplicatorConfig.maxProgress`.
pub const MAX_PROGRESS: i32 = 62;
/// `MobDuplicatorConfig.powerPerOperation`.
pub const POWER_PER_OPERATION: i32 = 5_000;
/// `MobDuplicatorConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 50_000;
/// `MobDuplicatorConfig.tankSize`.
pub const TANK_MB: i32 = 8_000;
/// The essence tank's fluid (`ModuleCore.ESSENCE`), a key this package declares.
pub const ESSENCE: &str = "industrialforegoing:essence";
/// `MobDuplicatorConfig.essenceNeeded`: mB per point of the spawned mob's health.
pub const ESSENCE_PER_HEALTH: i32 = 12;
/// `MobDuplicatorTile.work()`: more entities of the class than this in the area refuses.
pub const NEARBY_LIMIT: usize = 32;
/// `1 + random.nextInt(min(canSpawn, 4))`: the most one operation spawns.
pub const MAX_SPAWN: i32 = 4;
/// `SidedInventoryComponent("mob_imprisonment_tool", …, 1, 1)`: the input container's slots.
pub const INPUT_SLOTS: usize = 1;

/// What the glue learned about the tool and the working area this attempt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attempt {
    /// `entity.getHealth()` of the entity the tool holds (the fresh entity's health upstream).
    pub health: f32,
    /// `level.getEntitiesOfClass(entity.getClass(), area)` after removing the dead.
    pub nearby: usize,
    /// `level.random.nextInt(min(canSpawn, 4))`, already bounded by the glue.
    pub draw: i32,
}

/// The attempt's outcome, as the glue needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The bar is still counting down.
    Working,
    /// The bar reached zero and nothing was spawned (a missing tool, FE, essence, a blacklisted
    /// type, the nearby cap, or no whole spawn's worth of essence).
    Idle,
    /// The bar reached zero and `count` fresh mobs are to be spawned, each costing `essence_each`
    /// mB (the glue drains one per spawn it completes).
    Spawn {
        /// How many spawns the operation attempts.
        count: i32,
        /// The essence each spawned mob costs.
        essence_each: i32,
    },
}

/// The machine's numbers: the buffer, the bar, the essence tank and Titanium's
/// `progressIncrease` (last tick's step, 1 until a speed augment sets it).
#[derive(Debug, Clone, PartialEq)]
pub struct Duplicator {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; the machine starts it full and it counts down.
    pub progress: i32,
    /// The essence tank, `0..=TANK_MB`.
    pub essence_mb: i32,
    /// `ProgressBarComponent.progressIncrease`: this tick's step.
    pub increase: i32,
}

impl Default for Duplicator {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
            essence_mb: 0,
            increase: 1,
        }
    }
}

impl Duplicator {
    /// A machine from its saved compound; values outside the capacities are clamped, and a bar the
    /// save never wrote starts full (`ProgressBarComponent(maxProgress, maxProgress)`).
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>, essence_mb: i32) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
            essence_mb: essence_mb.clamp(0, TANK_MB),
            increase: 1,
        }
    }

    /// The bar's length with the seated augments: `floor(62 * efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        let scaled = (MAX_PROGRESS as f32 * augments.efficiency).floor() as i32;
        scaled.max(1)
    }

    /// One server tick: fall the bar by this tick's step, and at zero run one attempt.
    ///
    /// `attempt` is `None` when the tool slot is empty, the tank is empty, the tool's entity is not
    /// readable or its type is blacklisted. The glue spawns and drains when the answer is
    /// [`Step::Spawn`].
    pub fn tick(&mut self, augments: Augments, attempt: Option<Attempt>) -> Step {
        let bar = Self::bar_length(augments);
        self.progress = (self.progress - self.increase.max(1)).max(0);
        // `onTickWork` runs after the bar's own fall (`IndustrialWorkingTile`), so a speed augment
        // shows from the second tick — the same order the crusher and the chamber reproduce.
        self.increase = augments.speed.max(1);
        if self.progress > 0 {
            return Step::Working;
        }
        let Some(attempt) = attempt.filter(|attempt| attempt.nearby <= NEARBY_LIMIT) else {
            self.progress = bar;
            return Step::Idle;
        };
        let each = essence_for_health(attempt.health);
        if self.energy < POWER_PER_OPERATION || self.essence_mb <= 0 {
            self.progress = bar;
            return Step::Idle;
        }
        let can_spawn = self.essence_mb / each.max(1);
        if can_spawn == 0 {
            self.progress = bar;
            return Step::Idle;
        }
        self.energy -= POWER_PER_OPERATION;
        self.progress = bar;
        Step::Spawn {
            count: spawn_count(can_spawn, attempt.draw),
            essence_each: each,
        }
    }

    /// Take one spawned mob's essence out of the tank; the clamp is `drainForced`'s.
    pub fn spend_essence(&mut self, mb: i32) -> i32 {
        let taken = mb.max(0).min(self.essence_mb);
        self.essence_mb -= taken;
        taken
    }
}

/// `MobDuplicatorTile.work()`'s cost: `(int)(health * 12)` mB, a negative health as nothing.
#[must_use]
pub fn essence_for_health(health: f32) -> i32 {
    if !health.is_finite() || health <= 0.0 {
        return 0;
    }
    (health * ESSENCE_PER_HEALTH as f32) as i32
}

/// `1 + random.nextInt(min(canSpawn, 4))`: the operation's spawn count, zero when it cannot run.
#[must_use]
pub fn spawn_count(can_spawn: i32, draw: i32) -> i32 {
    if can_spawn <= 0 {
        return 0;
    }
    let bound = can_spawn.min(MAX_SPAWN);
    1 + draw.clamp(0, bound - 1)
}

/// The augments of a machine's slots, for the glue and the tests below.
#[must_use]
pub fn augments_of(slots: [Option<Augment>; AUGMENT_SLOTS]) -> Augments {
    Augments::from_slots(slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::AugmentType;

    fn speed(value: f32) -> Augment {
        Augment {
            kind: AugmentType::Speed,
            value,
        }
    }

    fn efficiency(value: f32) -> Augment {
        Augment {
            kind: AugmentType::Efficiency,
            value,
        }
    }

    /// A duplicator with `energy` FE and its bar one tick from the attempt.
    fn ready(energy: i32, mb: i32) -> Duplicator {
        Duplicator {
            energy,
            progress: 1,
            essence_mb: mb,
            ..Duplicator::default()
        }
    }

    fn cow() -> Attempt {
        Attempt {
            health: 10.0,
            nearby: 0,
            draw: 0,
        }
    }

    #[test]
    fn the_constants_are_the_config_s_own() {
        assert_eq!(MAX_PROGRESS, 62);
        assert_eq!(POWER_PER_OPERATION, 5_000);
        assert_eq!(CAPACITY_FE, 50_000);
        assert_eq!(TANK_MB, 8_000);
        assert_eq!(ESSENCE_PER_HEALTH, 12);
        assert_eq!(NEARBY_LIMIT, 32);
        assert_eq!(MAX_SPAWN, 4);
        assert_eq!(INPUT_SLOTS, 1);
    }

    #[test]
    fn a_cow_costs_a_hundred_and_twenty_mb() {
        assert_eq!(essence_for_health(10.0), 120);
        assert_eq!(essence_for_health(20.0), 240);
        assert_eq!(essence_for_health(4.0), 48);
        assert_eq!(
            essence_for_health(7.5),
            90,
            "Java's `(int)` truncates the product"
        );
        assert_eq!(essence_for_health(0.0), 0);
        assert_eq!(
            essence_for_health(-4.0),
            0,
            "a negative health gives nothing"
        );
    }

    #[test]
    fn one_spawn_costs_five_thousand_fe_and_rests_the_bar_full() {
        let mut duplicator = ready(10_000, 120);
        assert_eq!(
            duplicator.tick(Augments::NONE, Some(cow())),
            Step::Spawn {
                count: 1,
                essence_each: 120
            }
        );
        assert_eq!(duplicator.energy, 5_000, "5,000 FE an operation");
        assert_eq!(duplicator.progress, MAX_PROGRESS, "WorkAction(1f, …)");
    }

    #[test]
    fn the_tanks_worth_allows_up_to_four_and_the_draw_picks_between_them() {
        // 120 mB is exactly one; 240 gives one or two; 480 is the four-mob ceiling.
        assert_eq!(spawn_count(1, 0), 1);
        assert_eq!(spawn_count(2, 0), 1);
        assert_eq!(spawn_count(2, 1), 2);
        assert_eq!(spawn_count(3, 2), 3);
        assert_eq!(spawn_count(9, 0), 1);
        assert_eq!(spawn_count(9, 3), 4);
        assert_eq!(
            spawn_count(9, 99),
            4,
            "the draw is bounded by min(canSpawn, 4)"
        );
        assert_eq!(spawn_count(0, 0), 0);
        assert_eq!(spawn_count(-3, 0), 0);
        let mut duplicator = ready(10_000, 480);
        assert_eq!(
            duplicator.tick(Augments::NONE, Some(Attempt { draw: 3, ..cow() })),
            Step::Spawn {
                count: 4,
                essence_each: 120
            }
        );
    }

    #[test]
    fn without_a_tool_nothing_is_spent_and_the_bar_rests_full() {
        let mut duplicator = ready(50_000, 8_000);
        assert_eq!(duplicator.tick(Augments::NONE, None), Step::Idle);
        assert_eq!(duplicator.energy, 50_000);
        assert_eq!(duplicator.progress, MAX_PROGRESS);
    }

    #[test]
    fn without_five_thousand_fe_nothing_spawns() {
        let mut duplicator = ready(POWER_PER_OPERATION - 1, 8_000);
        assert_eq!(duplicator.tick(Augments::NONE, Some(cow())), Step::Idle);
        assert_eq!(duplicator.energy, 4_999, "hasEnergy(5000) refuses");
        assert_eq!(duplicator.progress, MAX_PROGRESS);
    }

    #[test]
    fn an_empty_tank_or_less_than_one_spawn_refuses_without_fe() {
        let mut empty = ready(10_000, 0);
        assert_eq!(empty.tick(Augments::NONE, Some(cow())), Step::Idle);
        assert_eq!(empty.energy, 10_000);
        let mut short = ready(10_000, 119);
        assert_eq!(short.tick(Augments::NONE, Some(cow())), Step::Idle);
        assert_eq!(short.energy, 10_000);
        assert_eq!(short.progress, MAX_PROGRESS);
    }

    #[test]
    fn more_than_thirty_two_of_the_same_class_refuses_without_fe() {
        let mut at_cap = ready(10_000, 8_000);
        assert_eq!(
            at_cap.tick(
                Augments::NONE,
                Some(Attempt {
                    nearby: NEARBY_LIMIT,
                    ..cow()
                })
            ),
            Step::Spawn {
                count: 1,
                essence_each: 120
            },
            "32 is allowed; only more than 32 refuses"
        );
        let mut over = ready(10_000, 8_000);
        assert_eq!(
            over.tick(
                Augments::NONE,
                Some(Attempt {
                    nearby: NEARBY_LIMIT + 1,
                    ..cow()
                })
            ),
            Step::Idle
        );
        assert_eq!(over.energy, 10_000);
        assert_eq!(over.progress, MAX_PROGRESS);
    }

    #[test]
    fn the_essence_drain_clamps_at_zero_and_counts_each_spawn() {
        let mut duplicator = ready(10_000, 240);
        assert_eq!(
            duplicator.tick(Augments::NONE, Some(Attempt { draw: 1, ..cow() })),
            Step::Spawn {
                count: 2,
                essence_each: 120
            }
        );
        assert_eq!(duplicator.spend_essence(120), 120);
        assert_eq!(duplicator.essence_mb, 120);
        assert_eq!(duplicator.spend_essence(120), 120);
        assert_eq!(duplicator.essence_mb, 0);
        assert_eq!(duplicator.spend_essence(120), 0, "drainForced clamps");
        assert_eq!(duplicator.essence_mb, 0);
    }

    #[test]
    fn an_efficiency_augment_shortens_the_bar() {
        let augments = augments_of([None, Some(efficiency(0.8))]);
        assert_eq!(Duplicator::bar_length(augments), 49, "floor(62 * 0.8)");
        let mut duplicator = ready(10_000, 120);
        assert_eq!(
            duplicator.tick(augments, Some(cow())),
            Step::Spawn {
                count: 1,
                essence_each: 120
            }
        );
        assert_eq!(duplicator.progress, 49);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_faster_from_the_second_tick() {
        let augments = augments_of([Some(speed(4.0)), None]);
        let mut duplicator = Duplicator {
            energy: 10_000,
            progress: 20,
            essence_mb: 120,
            ..Duplicator::default()
        };
        assert_eq!(duplicator.tick(augments, Some(cow())), Step::Working);
        assert_eq!(duplicator.progress, 19);
        assert_eq!(duplicator.increase, 4);
        assert_eq!(duplicator.tick(augments, Some(cow())), Step::Working);
        assert_eq!(duplicator.progress, 15, "4 a tick from the second tick");
    }

    #[test]
    fn a_bar_the_save_never_wrote_starts_full() {
        let mut duplicator = Duplicator::from_saved(0, None, 0);
        assert_eq!(duplicator.progress, MAX_PROGRESS);
        assert_eq!(duplicator.tick(Augments::NONE, None), Step::Working);
        assert_eq!(duplicator.progress, MAX_PROGRESS - 1);
        let saved = Duplicator::from_saved(90_000, Some(-3), 90_000);
        assert_eq!(saved.energy, CAPACITY_FE);
        assert_eq!(saved.progress, 0);
        assert_eq!(saved.essence_mb, TANK_MB);
    }
}
