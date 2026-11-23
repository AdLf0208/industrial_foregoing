//! The Mob Crusher's rules (`MobCrusherTile`), pure and unit-tested.
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile`: a bar that counts **down** from
//! `maxProgress` 100, and at zero one attempt. The attempt needs **50 FE**
//! (`MobCrusherConfig.powerPerOperation`) and a **Mob** in the working area — the block behind
//! the machine, expanded two blocks up (`RangeManager.RangeType.BEHIND` + `expandTowards(0, 2,
//! 0)`) — that is alive, not `isInvulnerable()`, not a baby animal, and not a wither still in its
//! spawn invulnerability. The first such mob is killed; a type in the jar's
//! `mob_crusher_instant_kill_blacklist` (the ender dragon) is hurt for
//! `MobCrusherConfig.attackDamage` 75 instead.
//!
//! A kill of either kind spends 50 FE and rests the bar at `floor(barLength * 0.1)`; an attempt
//! with no mob, or with less than 50 FE, answers `WorkAction(1f, 0)` — nothing is killed,
//! **nothing is spent**, and the bar rests full.
//!
//! # The essence
//!
//! The jar fills its tank with the victim's experience: `experience * 20` mB, where
//! `experience = entity.getExperienceReward(level, fakePlayer)`. The rule is therefore
//! **20 mB per experience point**, not per health; [`essence_for`] is that one multiplication and
//! the tests pin the numbers a cow's 1–3 points give (20, 40, 60 mB).
//!
//! Everything the game supplies — the FE, the bar, the tank — is this module's state; the glue
//! (`crate::mobcrusher`) reads and writes it.

use crate::runtime::{AUGMENT_SLOTS, Augment, Augments};

/// `MobCrusherConfig.maxProgress`.
pub const MAX_PROGRESS: i32 = 100;
/// `MobCrusherConfig.powerPerOperation`.
pub const POWER_PER_OPERATION: i32 = 50;
/// `MobCrusherConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `MobCrusherConfig.tankSize`.
pub const TANK_MB: i32 = 32_000;
/// The essence tank's fluid (`ModuleCore.ESSENCE`), a key this package declares.
pub const ESSENCE: &str = "industrialforegoing:essence";
/// The working area's extra height: `getBox().expandTowards(0, 2, 0)`.
pub const AREA_UP: i32 = 2;
/// `WorkAction.workAmount` after a kill: the bar rests at a tenth of its length.
pub const KILL_WORK_AMOUNT: f32 = 0.1;
/// `MobCrusherConfig.attackDamage`: the damage the blacklist path deals.
pub const ATTACK_DAMAGE: f32 = 75.0;
/// `tank.fillForced(… experience * 20)`: one experience point is this much essence.
pub const ESSENCE_PER_POINT: i32 = 20;
/// `SidedInventoryComponent("output", …, 6 * 3, 1)`: the output container's slots.
pub const OUTPUT_SLOTS: usize = 18;

/// One entity in the working area, as the jar's filter reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// `entity instanceof Mob` (players and items are not mobs).
    pub mob: bool,
    /// `LivingEntity.isAlive()`.
    pub alive: bool,
    /// `Entity.isInvulnerable()` — the `Invulnerable` flag.
    pub invulnerable: bool,
    /// `Mob.isBaby()`.
    pub baby: bool,
    /// `entity instanceof Animal`.
    pub animal: bool,
    /// `WitherBoss.getInvulnerableTicks() > 0`, its spawn invulnerability.
    pub wither_spawning: bool,
    /// The entity type is in the jar's `mob_crusher_instant_kill_blacklist` (the ender dragon),
    /// which sends the attempt down `damage()` instead of `instantKill()`. The filter does not
    /// read it; the glue sets it from the entity-type tag and [`Step::Killed`] carries it back.
    pub blacklisted: bool,
}

/// `MobCrusherTile.work()`'s filter, in the order it applies them.
#[must_use]
pub fn eligible(candidate: Candidate) -> bool {
    candidate.mob
        && candidate.alive
        && !candidate.invulnerable
        && !(candidate.animal && candidate.baby)
        && !candidate.wither_spawning
}

/// The jar's two kill paths: an instant kill, or 75 damage for a blacklisted type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackPath {
    /// `instantKill`: the loot table and the experience, as a player kill.
    Instant,
    /// `damage`: `entity.hurt(damageSources().playerAttack(player), 75)`.
    Blacklisted,
}

/// The attempt's outcome, as the glue needs it: an idle bar, a bar on its way down, or a kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The bar reached zero and nothing was killed (no mob, or under 50 FE).
    Idle,
    /// The bar is still counting down.
    Working,
    /// The bar reached zero and the victim is to be hit, by the jar's [path](AttackPath).
    Killed {
        /// Which of the jar's two kill paths the victim takes.
        path: AttackPath,
    },
}

/// The machine's numbers: the buffer, the bar, the essence tank and Titanium's
/// `progressIncrease` (last tick's step, 1 until a speed augment sets it).
#[derive(Debug, Clone, PartialEq)]
pub struct Crusher {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; the machine starts it full and it counts down.
    pub progress: i32,
    /// The essence tank, `0..=TANK_MB`.
    pub essence_mb: i32,
    /// `ProgressBarComponent.progressIncrease`: this tick's step.
    pub increase: i32,
}

impl Default for Crusher {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
            essence_mb: 0,
            increase: 1,
        }
    }
}

impl Crusher {
    /// A machine from its saved compound; values outside the capacities are clamped, and a bar
    /// the save never wrote starts full (`ProgressBarComponent(maxProgress, maxProgress)`).
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>, essence_mb: i32) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
            essence_mb: essence_mb.clamp(0, TANK_MB),
            increase: 1,
        }
    }

    /// The bar's length with the seated augments: `floor(100 * efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        let scaled = (MAX_PROGRESS as f32 * augments.efficiency).floor() as i32;
        scaled.max(1)
    }

    /// The ticks a completed attempt rests for: `floor(bar length * workAmount)`.
    #[must_use]
    pub fn rest(bar: i32, work_amount: f32) -> i32 {
        (bar as f32 * work_amount).floor() as i32
    }

    /// One server tick: fall the bar by this tick's step, and at zero run one attempt.
    ///
    /// `victim` is the eligible mob the glue found in the working area, if any; the glue hits it
    /// when the answer is [`Step::Killed`].
    pub fn tick(&mut self, augments: Augments, victim: Option<Candidate>) -> Step {
        let bar = Self::bar_length(augments);
        self.progress = (self.progress - self.increase.max(1)).max(0);
        // `onTickWork` runs after the bar's own fall, so a speed augment shows from the second
        // tick — the same order the chamber, the latex chain and the slaughter factory reproduce.
        self.increase = augments.speed.max(1);
        if self.progress > 0 {
            return Step::Working;
        }
        match victim {
            Some(victim) if self.energy >= POWER_PER_OPERATION && eligible(victim) => {
                self.energy -= POWER_PER_OPERATION;
                self.progress = Self::rest(bar, KILL_WORK_AMOUNT);
                Step::Killed {
                    path: if victim.blacklisted {
                        AttackPath::Blacklisted
                    } else {
                        AttackPath::Instant
                    },
                }
            }
            _ => {
                self.progress = bar;
                Step::Idle
            }
        }
    }

    /// One harvested kill's experience: `experience * 20` mB into the tank, clamped at 32,000
    /// (`fillForced` clamps; a full tank still counts the kill).
    pub fn deposit_essence(&mut self, points: i32) -> i32 {
        let gained = essence_for(points);
        self.essence_mb = (self.essence_mb + gained).clamp(0, TANK_MB);
        gained
    }
}

/// `MobCrusherTile.instantKill`'s essence: `experience * 20` mB, negative points as nothing.
#[must_use]
pub fn essence_for(points: i32) -> i32 {
    points.max(0) * ESSENCE_PER_POINT
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

    fn cow() -> Candidate {
        Candidate {
            mob: true,
            alive: true,
            invulnerable: false,
            baby: false,
            animal: true,
            wither_spawning: false,
            blacklisted: false,
        }
    }

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

    /// A crusher with `energy` FE and its bar one tick from the attempt.
    fn ready(energy: i32) -> Crusher {
        Crusher {
            energy,
            progress: 1,
            ..Crusher::default()
        }
    }

    #[test]
    fn a_cow_is_eligible_and_a_baby_or_an_invulnerable_mob_is_not() {
        assert!(eligible(cow()));
        assert!(
            !eligible(Candidate {
                baby: true,
                ..cow()
            }),
            "an Animal that isBaby() is skipped"
        );
        assert!(
            eligible(Candidate {
                animal: false,
                baby: true,
                ..cow()
            }),
            "a baby non-animal (a baby zombie) is a mob like any other"
        );
        assert!(!eligible(Candidate {
            invulnerable: true,
            ..cow()
        }));
        assert!(!eligible(Candidate {
            alive: false,
            ..cow()
        }));
        assert!(!eligible(Candidate {
            mob: false,
            ..cow()
        }));
        assert!(
            !eligible(Candidate {
                wither_spawning: true,
                ..cow()
            }),
            "a wither still in its spawn invulnerability is skipped"
        );
    }

    #[test]
    fn one_kill_costs_fifty_fe_and_rests_the_bar_at_ten() {
        let mut crusher = ready(1_000);
        assert_eq!(
            crusher.tick(Augments::NONE, Some(cow())),
            Step::Killed {
                path: AttackPath::Instant
            }
        );
        assert_eq!(crusher.energy, 950, "50 FE an operation");
        assert_eq!(crusher.progress, 10, "floor(100 * 0.1)");
    }

    #[test]
    fn without_a_mob_nothing_is_killed_and_nothing_is_spent() {
        let mut crusher = ready(5_000);
        assert_eq!(crusher.tick(Augments::NONE, None), Step::Idle);
        assert_eq!(crusher.energy, 5_000, "`WorkAction(1f, 0)` costs nothing");
        assert_eq!(crusher.progress, MAX_PROGRESS, "the bar rests full");
    }

    #[test]
    fn without_fifty_fe_the_mob_survives() {
        let mut crusher = ready(POWER_PER_OPERATION - 1);
        assert_eq!(crusher.tick(Augments::NONE, Some(cow())), Step::Idle);
        assert_eq!(
            crusher.energy, 49,
            "hasEnergy(50) is false, so nothing is taken"
        );
        assert_eq!(crusher.progress, MAX_PROGRESS);
    }

    #[test]
    fn a_blacklisted_type_is_marked_for_the_damage_path() {
        let mut crusher = ready(1_000);
        assert_eq!(
            crusher.tick(Augments::NONE, Some(cow())),
            Step::Killed {
                path: AttackPath::Instant
            }
        );
        // The ender dragon is the jar's one blacklisted type: the attempt still costs and rests,
        // and the answer says the glue must hurt it for 75 instead of killing it outright.
        crusher.energy = 1_000;
        crusher.progress = 1;
        assert_eq!(
            crusher.tick(
                Augments::NONE,
                Some(Candidate {
                    blacklisted: true,
                    ..cow()
                })
            ),
            Step::Killed {
                path: AttackPath::Blacklisted
            }
        );
        assert_eq!(crusher.energy, 950);
        assert_eq!(ATTACK_DAMAGE, 75.0, "MobCrusherConfig.attackDamage");
        // A maxed tank still costs the operation its FE.
        crusher.energy = 1_000;
        crusher.essence_mb = TANK_MB - 5;
        assert_eq!(crusher.deposit_essence(2), 40);
        assert_eq!(crusher.essence_mb, TANK_MB);
    }

    #[test]
    fn essence_is_twenty_mb_a_point() {
        assert_eq!(essence_for(1), 20, "a cow's low roll");
        assert_eq!(essence_for(2), 40, "a cow's middle roll");
        assert_eq!(essence_for(3), 60, "a cow's high roll");
        assert_eq!(essence_for(0), 0);
        assert_eq!(essence_for(-4), 0, "a negative reward gives nothing");
        assert_eq!(
            essence_for(10),
            200,
            "a fixed 10-point mob: 200 mB, not 20 × its health"
        );
    }

    #[test]
    fn the_tank_clamps_at_thirty_two_thousand_mb() {
        let mut crusher = Crusher {
            essence_mb: TANK_MB - 30,
            ..Crusher::default()
        };
        assert_eq!(crusher.deposit_essence(3), 60, "the kill still counts");
        assert_eq!(crusher.essence_mb, TANK_MB, "fillForced clamps");
    }

    #[test]
    fn an_efficiency_augment_shortens_the_bar_and_the_rest() {
        let augments = augments_of([None, Some(efficiency(0.8))]);
        assert_eq!(Crusher::bar_length(augments), 80, "floor(100 * 0.8)");
        let mut crusher = Crusher {
            energy: 1_000,
            progress: 1,
            ..Crusher::default()
        };
        assert_eq!(
            crusher.tick(augments, Some(cow())),
            Step::Killed {
                path: AttackPath::Instant
            }
        );
        assert_eq!(crusher.progress, 8, "floor(80 * 0.1)");
        crusher.progress = 1;
        assert_eq!(crusher.tick(augments, None), Step::Idle);
        assert_eq!(crusher.progress, 80);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_faster() {
        let augments = augments_of([Some(speed(4.0)), None]);
        let mut crusher = Crusher {
            energy: 1_000,
            progress: 20,
            ..Crusher::default()
        };
        // The first tick still uses the initial step of 1 (`onTickWork` sets the rate after the
        // fall), so the bar is at 19 and only then does the augment show.
        assert_eq!(crusher.tick(augments, Some(cow())), Step::Working);
        assert_eq!(crusher.progress, 19);
        assert_eq!(crusher.increase, 4);
        assert_eq!(crusher.tick(augments, Some(cow())), Step::Working);
        assert_eq!(crusher.progress, 15, "4 a tick from the second tick");
    }

    #[test]
    fn a_bar_the_save_never_wrote_starts_full() {
        let mut crusher = Crusher::from_saved(0, None, 0);
        assert_eq!(
            crusher.progress, MAX_PROGRESS,
            "ProgressBarComponent starts full"
        );
        assert_eq!(crusher.tick(Augments::NONE, None), Step::Working);
        assert_eq!(crusher.progress, MAX_PROGRESS - 1);
        crusher.progress = 1;
        assert_eq!(crusher.tick(Augments::NONE, None), Step::Idle);
        assert_eq!(crusher.progress, MAX_PROGRESS, "an idle attempt rests full");
        let saved = Crusher::from_saved(50_000, Some(-3), 90_000);
        assert_eq!(saved.energy, CAPACITY_FE);
        assert_eq!(saved.progress, 0);
        assert_eq!(saved.essence_mb, TANK_MB);
    }
}
