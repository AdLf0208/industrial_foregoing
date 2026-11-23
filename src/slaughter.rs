//! The Mob Slaughter Factory's rules (`SlaughterFactoryTile`), pure and unit-tested.
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile`: a bar that counts **down** from
//! `maxProgress` 100, and at zero one attempt. The attempt needs **400 FE** in the buffer and a
//! **Mob** in the working area — the block behind the machine, expanded two blocks up
//! (`RangeManager.RangeType.BEHIND` + `expandTowards(0, 2, 0)`). The first alive, not permanently
//! invulnerable mob is removed `KILLED` and its health becomes the two tanks' contents:
//!
//! ```text
//! animal      meat = (int) health            pink slime = (int) (health * 20)
//! non-animal  meat = (int) health * 20       pink slime = (int) health
//! ```
//!
//! The two expressions are the jar's own, and they are **not** the same cast: the animal's slime
//! truncates the product, the non-animal's meat truncates the health and then multiplies. The
//! tests below pin both, with a fractional health, so a "tidy" rewrite that makes them agree goes
//! red.
//!
//! The attempt's cost is `WorkAction(0.2f, 400)`: 400 FE out of the buffer and the bar rests at
//! `floor(barLength * 0.2)`. With no mob, or with less than 400 FE, `work()` answers
//! `WorkAction(1f, 0)` — nothing is killed, **nothing is spent**, and the bar rests full.
//!
//! Everything the game supplies — the FE, the bar, the two tanks — is this module's state; the
//! glue (`crate::mobslaughter`) reads and writes it.

use crate::runtime::{AUGMENT_SLOTS, Augment, Augments};

/// `SlaughterFactoryConfig.maxProgress`.
pub const MAX_PROGRESS: i32 = 100;
/// `SlaughterFactoryConfig.powerPerOperation`.
pub const POWER_PER_OPERATION: i32 = 400;
/// `SlaughterFactoryConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `SlaughterFactoryConfig.maxMeatTankSize`.
pub const MEAT_TANK_MB: i32 = 8_000;
/// `SlaughterFactoryConfig.maxPinkSlimeTankSize`.
pub const PINK_SLIME_TANK_MB: i32 = 8_000;
/// The meat tank's fluid (`ModuleCore.MEAT`), a key this package declares.
pub const MEAT: &str = "industrialforegoing:meat";
/// The pink slime tank's fluid (`ModuleCore.PINK_SLIME`).
pub const PINK_SLIME: &str = "industrialforegoing:pink_slime";
/// The working area's extra height: `getBox().expandTowards(0, 2, 0)`.
pub const AREA_UP: i32 = 2;
/// `WorkAction.workAmount` after a kill: the bar rests at a fifth of its length.
pub const KILL_WORK_AMOUNT: f32 = 0.2;

/// One entity the machine may kill: the two things the jar's `work()` reads off it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Victim {
    /// `LivingEntity.getHealth()` at the moment of the kill.
    pub health: f32,
    /// `entity instanceof Animal`.
    pub animal: bool,
}

/// What one kill puts in the two tanks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Yields {
    /// mB for the meat tank.
    pub meat_mb: i32,
    /// mB for the pink slime tank.
    pub pink_slime_mb: i32,
}

/// The attempt's outcome, as the glue needs it: an idle bar, a bar on its way down, or a kill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The bar reached zero and nothing was killed (no mob, or under 400 FE).
    Idle,
    /// The bar is still counting down.
    Working,
    /// The bar reached zero and the victim was killed for these amounts.
    Killed(Yields),
}

/// The two tanks' amounts and the attempt's two numbers: the bar, the FE, the tanks, and
/// Titanium's `progressIncrease` (last tick's step, 1 until a speed augment sets it).
#[derive(Debug, Clone, PartialEq)]
pub struct Factory {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; the machine starts it full and it counts down.
    pub progress: i32,
    /// The meat tank, `0..=MEAT_TANK_MB`.
    pub meat_mb: i32,
    /// The pink slime tank, `0..=PINK_SLIME_TANK_MB`.
    pub pink_slime_mb: i32,
    /// `ProgressBarComponent.progressIncrease`: this tick's step.
    pub increase: i32,
}

impl Default for Factory {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
            meat_mb: 0,
            pink_slime_mb: 0,
            increase: 1,
        }
    }
}

impl Factory {
    /// A machine from its saved compound; values outside the capacities are clamped, and a bar
    /// the save never wrote starts full (`ProgressBarComponent(maxProgress, maxProgress)`).
    #[must_use]
    pub fn from_saved(
        energy: i32,
        progress: Option<i32>,
        meat_mb: i32,
        pink_slime_mb: i32,
    ) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
            meat_mb: meat_mb.clamp(0, MEAT_TANK_MB),
            pink_slime_mb: pink_slime_mb.clamp(0, PINK_SLIME_TANK_MB),
            increase: 1,
        }
    }

    /// The bar's length with the seated augments: `floor(100 * efficiency)`, never below 1
    /// (Titanium's `onFinishWork` sets `maxProgress` from the efficiency augment).
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
    /// `victim` is the mob the glue found in the working area, if any; the glue kills it when the
    /// answer is [`Step::Killed`]. [`Self::tick_with`] is the seam the glue uses so the
    /// working-area scan is deferred until a tick really attempts work.
    pub fn tick(&mut self, augments: Augments, victim: Option<Victim>) -> Step {
        self.tick_with(augments, || victim)
    }

    /// One server tick with the working-area scan supplied as a closure.
    ///
    /// Upstream reaches `getBox()`/`entities` inside `work()`, and `onFinishWork` calls `work()`
    /// only when the bar has reached zero **and** the buffer holds the operation's FE — so a
    /// machine that is still counting down, or cannot pay, never touches the entity list.
    pub fn tick_with<F>(&mut self, augments: Augments, scan: F) -> Step
    where
        F: FnOnce() -> Option<Victim>,
    {
        let bar = Self::bar_length(augments);
        let step = self.increase.max(1);
        // `onFinishWork` reaches `work()` only at zero and only with the operation's FE, so this
        // is the one tick that may look at the working area.
        let attempt = self.progress - step <= 0 && self.energy >= POWER_PER_OPERATION;
        self.progress = (self.progress - step).max(0);
        // `onTickWork` runs after the bar's own fall, so a speed augment shows from the second
        // tick — the same order the chamber and the latex chain reproduce.
        self.increase = augments.speed.max(1);
        if self.progress > 0 {
            return Step::Working;
        }
        if !attempt {
            self.progress = bar;
            return Step::Idle;
        }
        match scan() {
            Some(victim) => {
                self.energy -= POWER_PER_OPERATION;
                let yields = yields_for(victim);
                self.meat_mb = (self.meat_mb + yields.meat_mb).clamp(0, MEAT_TANK_MB);
                self.pink_slime_mb =
                    (self.pink_slime_mb + yields.pink_slime_mb).clamp(0, PINK_SLIME_TANK_MB);
                self.progress = Self::rest(bar, KILL_WORK_AMOUNT);
                Step::Killed(yields)
            }
            _ => {
                self.progress = bar;
                Step::Idle
            }
        }
    }
}

/// What one kill puts in the tanks: the jar's two expressions, faithfully.
#[must_use]
pub fn yields_for(victim: Victim) -> Yields {
    let health = victim.health.max(0.0);
    // `(int) currentHealth` — Java truncates toward zero; Rust's `as` does the same for a
    // non-negative float.
    let whole = health as i32;
    if victim.animal {
        // `(int) currentHealth` and `(int) (currentHealth * 20)`.
        Yields {
            meat_mb: whole,
            pink_slime_mb: (health * 20.0) as i32,
        }
    } else {
        // `(int) currentHealth * 20` — the cast binds before the multiply — and
        // `(int) currentHealth`.
        Yields {
            meat_mb: whole * 20,
            pink_slime_mb: whole,
        }
    }
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

    fn cow() -> Victim {
        Victim {
            health: 10.0,
            animal: true,
        }
    }

    fn villager() -> Victim {
        Victim {
            health: 20.0,
            animal: false,
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

    fn augments_of_slots(slots: [Option<Augment>; AUGMENT_SLOTS]) -> Augments {
        augments_of(slots)
    }

    /// A factory with `energy` FE and its bar one tick from the attempt.
    fn ready(energy: i32) -> Factory {
        Factory {
            energy,
            progress: 1,
            ..Factory::default()
        }
    }

    #[test]
    fn a_cow_gives_ten_meat_and_eighty_slime() {
        assert_eq!(
            yields_for(cow()),
            Yields {
                meat_mb: 10,
                pink_slime_mb: 200
            },
            "an animal: (int) health meat, (int)(health * 20) slime"
        );
        // The two casts differ: the animal's slime truncates the product.
        assert_eq!(
            yields_for(Victim {
                health: 9.7,
                animal: true
            }),
            Yields {
                meat_mb: 9,
                pink_slime_mb: 194
            },
            "9.7 health: 9 meat, (int)(9.7 * 20) = 194 slime"
        );
    }

    #[test]
    fn a_villager_gives_twenty_slime_and_four_hundred_meat() {
        assert_eq!(
            yields_for(villager()),
            Yields {
                meat_mb: 400,
                pink_slime_mb: 20
            },
            "a non-animal: (int) health * 20 meat, (int) health slime"
        );
        // `(int) heart * 20`, not `(int) (heart * 20)`: 19.7 gives 19 * 20 = 380, not 394.
        assert_eq!(
            yields_for(Victim {
                health: 19.7,
                animal: false
            }),
            Yields {
                meat_mb: 380,
                pink_slime_mb: 19
            },
            "the jar casts the health before multiplying, so 19.7 is 380"
        );
    }

    #[test]
    fn a_kill_costs_four_hundred_fe_and_rests_the_bar_at_twenty() {
        let mut factory = ready(1_000);
        assert_eq!(
            factory.tick(Augments::NONE, Some(cow())),
            Step::Killed(Yields {
                meat_mb: 10,
                pink_slime_mb: 200
            })
        );
        assert_eq!(factory.energy, 600, "400 FE an operation");
        assert_eq!(factory.progress, 20, "floor(100 * 0.2)");
        assert_eq!((factory.meat_mb, factory.pink_slime_mb), (10, 200));
    }

    #[test]
    fn without_a_mob_nothing_is_killed_and_nothing_is_spent() {
        let mut factory = ready(5_000);
        assert_eq!(factory.tick(Augments::NONE, None), Step::Idle);
        assert_eq!(factory.energy, 5_000, "`WorkAction(1f, 0)` costs nothing");
        assert_eq!(factory.progress, MAX_PROGRESS, "the bar rests full");
        assert_eq!((factory.meat_mb, factory.pink_slime_mb), (0, 0));
    }

    #[test]
    fn without_four_hundred_fe_the_mob_survives() {
        let mut factory = ready(POWER_PER_OPERATION - 1);
        assert_eq!(factory.tick(Augments::NONE, Some(cow())), Step::Idle);
        assert_eq!(
            factory.energy, 399,
            "`hasEnergy(400)` is false, so nothing is taken"
        );
        assert_eq!(factory.progress, MAX_PROGRESS);
        assert_eq!((factory.meat_mb, factory.pink_slime_mb), (0, 0));
    }

    #[test]
    fn the_tanks_clamp_at_eight_thousand_mb_each() {
        let mut factory = ready(1_000);
        factory.meat_mb = MEAT_TANK_MB - 5;
        factory.pink_slime_mb = PINK_SLIME_TANK_MB - 5;
        assert_eq!(
            factory.tick(Augments::NONE, Some(cow())),
            Step::Killed(Yields {
                meat_mb: 10,
                pink_slime_mb: 200
            })
        );
        assert_eq!(factory.meat_mb, MEAT_TANK_MB);
        assert_eq!(factory.pink_slime_mb, PINK_SLIME_TANK_MB);
        assert_eq!(
            factory.energy, 600,
            "a full tank still costs the operation its FE"
        );
    }

    #[test]
    fn an_efficiency_augment_shortens_the_bar_and_the_rest() {
        let augments = augments_of_slots([None, Some(efficiency(0.8))]);
        assert_eq!(Factory::bar_length(augments), 80, "floor(100 * 0.8)");
        let mut factory = Factory {
            energy: 1_000,
            progress: 1,
            ..Factory::default()
        };
        assert_eq!(
            factory.tick(augments, Some(cow())),
            Step::Killed(Yields {
                meat_mb: 10,
                pink_slime_mb: 200
            })
        );
        assert_eq!(factory.progress, 16, "floor(80 * 0.2)");
        // An idle attempt rests the full shortened bar.
        factory.progress = 1;
        assert_eq!(factory.tick(augments, None), Step::Idle);
        assert_eq!(factory.progress, 80);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_faster() {
        let augments = augments_of_slots([Some(speed(4.0)), None]);
        let mut factory = Factory {
            energy: 1_000,
            progress: 20,
            ..Factory::default()
        };
        // The first tick still uses the initial step of 1 (`onTickWork` sets the rate after the
        // fall), so the bar is at 19 and only then does the augment show.
        assert_eq!(factory.tick(augments, Some(cow())), Step::Working);
        assert_eq!(factory.progress, 19);
        assert_eq!(factory.increase, 4);
        assert_eq!(factory.tick(augments, Some(cow())), Step::Working);
        assert_eq!(factory.progress, 15, "4 a tick from the second tick");
    }

    #[test]
    fn a_bar_the_save_never_wrote_starts_full() {
        let mut factory = Factory::from_saved(0, None, 0, 0);
        assert_eq!(
            factory.progress, MAX_PROGRESS,
            "`ProgressBarComponent` starts full"
        );
        assert_eq!(factory.tick(Augments::NONE, None), Step::Working);
        assert_eq!(factory.progress, MAX_PROGRESS - 1);
        factory.progress = 1;
        assert_eq!(factory.tick(Augments::NONE, None), Step::Idle);
        assert_eq!(factory.progress, MAX_PROGRESS, "an idle attempt rests full");
        let saved = Factory::from_saved(50_000, Some(-3), 90_000, 90_000);
        assert_eq!(saved.energy, CAPACITY_FE);
        assert_eq!(saved.progress, 0);
        assert_eq!(saved.meat_mb, MEAT_TANK_MB);
        assert_eq!(saved.pink_slime_mb, PINK_SLIME_TANK_MB);
    }

    /// The working-area query is the expensive half of a tick, and upstream reaches it inside
    /// `work()` — which runs only at zero with the FE in hand.
    #[test]
    fn the_working_area_is_queried_only_on_a_powered_attempt() {
        let mut idle = Factory::default();
        let mut scans = 0;
        for _ in 0..5 {
            assert_eq!(
                idle.tick_with(Augments::NONE, || {
                    scans += 1;
                    None
                }),
                Step::Working
            );
        }
        assert_eq!(
            scans, 0,
            "an unpowered machine must not query the working area"
        );

        let mut falling = Factory {
            energy: 400,
            progress: 50,
            ..Factory::default()
        };
        let mut scans = 0;
        assert_eq!(
            falling.tick_with(Augments::NONE, || {
                scans += 1;
                None
            }),
            Step::Working
        );
        assert_eq!(scans, 0, "a bar still counting down does not query either");

        let mut ready_factory = Factory {
            energy: 400,
            progress: 1,
            ..Factory::default()
        };
        let mut scans = 0;
        let step = ready_factory.tick_with(Augments::NONE, || {
            scans += 1;
            Some(cow())
        });
        assert_eq!(scans, 1, "the attempt ticks once");
        assert!(matches!(step, Step::Killed(_)));
    }
}
