//! The Animal Rancher's rules (`AnimalRancherTile`), pure and unit-tested.
//!
//! Upstream's tile is an `IndustrialAreaWorkingTile(BEHIND, acceptsRangeUpgrades = true)`: a bar
//! that counts **down** from `maxProgress` **100** and, at zero, one attempt over every
//! `PathfinderMob` in the **single block behind** the machine. The attempt needs **400 FE**
//! (`AnimalRancherConfig.powerPerOperation`, checked before the loop, so no FE means nothing at
//! all), and takes the first of three arms that answers, in the jar's order:
//!
//! 1. **shearing** a `Shearable` mob — the sheep: `1 + random.nextInt(3)` wool of the sheep's
//!    colour into the 15-slot output container, and the sheep is left sheared;
//! 2. a **squid**: one black dye into the output while the container has a free slot and four
//!    `random.nextBoolean()` draws all pass (`1/16`);
//! 3. **milking** an `Animal` with room in the tank (`amount + 1000 <= 8,000`): a fresh bucket in a
//!    fake player's hand is filled with **1,000 mB** of milk and drained into the tank.
//!
//! A successful arm answers `WorkAction(0.35, 400)`: 400 FE spent and the bar rested at
//! `floor(maxProgress × 0.35)`. Nothing answered answers `WorkAction(1, 0)`: **no** FE and a full
//! bar. The speed augment falls the bar faster and the efficiency one shortens it, as Titanium's
//! `IndustrialWorkingTile` does; Titanium sets `progressIncrease` in `onTickWork` *after* the bar's
//! own `setProgress`, so its first tick after seating a speed addon falls 1 — the port falls by the
//! addon from the first tick, the model this port's condensator has used since slice 1.
//!
//! Everything the game supplies — the FE, the bar, the tank, what stands in the working area and
//! the two random draws — is this module's input; the glue (`crate::animalrancher`) reads and
//! writes it.

use crate::runtime::{AUGMENT_SLOTS, Augment, Augments};

/// `AnimalRancherConfig.maxProgress`.
pub const MAX_PROGRESS: i32 = 100;
/// `AnimalRancherConfig.powerPerOperation`.
pub const POWER_PER_OPERATION: i32 = 400;
/// `AnimalRancherConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `AnimalRancherConfig.maxTankSize`.
pub const TANK_MB: i32 = 8_000;
/// `SidedInventoryComponent("output", …, 5 * 3, 1)`: the output container's slots.
pub const OUTPUT_SLOTS: usize = 15;
/// One milking: the bucket the fake player fills.
pub const MILK_MB: i32 = 1_000;
/// `WorkAction(0.35f, …)`: the fraction of the bar a successful operation leaves.
pub const WORK_AMOUNT: f32 = 0.35;
/// `WorkAction(1f, 0)`: the fraction a refused or idle attempt leaves (the whole bar).
pub const IDLE_AMOUNT: f32 = 1.0;
/// `1 + random.nextInt(3)` wool a shearing gives.
pub const WOOL_EXTRA: i32 = 3;

/// The milk tank's fluid, a key this package declares: 26.3 has no milk in its fluid registry
/// (three entries: empty, water, lava), where NeoForge's `minecraft:milk` is what the jar's bucket
/// interaction drains. Recorded as a deviation in `notes.md`.
pub const MILK: &str = "industrialforegoing:milk";

/// `DyeColor` in vanilla's order: a sheep's saved `Color` is this index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wool {
    /// `DyeColor.WHITE`.
    White,
    /// `DyeColor.ORANGE`.
    Orange,
    /// `DyeColor.MAGENTA`.
    Magenta,
    /// `DyeColor.LIGHT_BLUE`.
    LightBlue,
    /// `DyeColor.YELLOW`.
    Yellow,
    /// `DyeColor.LIME`.
    Lime,
    /// `DyeColor.PINK`.
    Pink,
    /// `DyeColor.GRAY`.
    Gray,
    /// `DyeColor.LIGHT_GRAY`.
    LightGray,
    /// `DyeColor.CYAN`.
    Cyan,
    /// `DyeColor.PURPLE`.
    Purple,
    /// `DyeColor.BLUE`.
    Blue,
    /// `DyeColor.BROWN`.
    Brown,
    /// `DyeColor.GREEN`.
    Green,
    /// `DyeColor.RED`.
    Red,
    /// `DyeColor.BLACK`.
    Black,
}

impl Wool {
    /// The colour a sheep's saved `Color` byte names; anything outside `0..=15` is white, the
    /// default a fresh sheep has.
    #[must_use]
    pub fn from_colour(colour: i32) -> Self {
        match colour {
            1 => Self::Orange,
            2 => Self::Magenta,
            3 => Self::LightBlue,
            4 => Self::Yellow,
            5 => Self::Lime,
            6 => Self::Pink,
            7 => Self::Gray,
            8 => Self::LightGray,
            9 => Self::Cyan,
            10 => Self::Purple,
            11 => Self::Blue,
            12 => Self::Brown,
            13 => Self::Green,
            14 => Self::Red,
            15 => Self::Black,
            _ => Self::White,
        }
    }

    /// The wool item this colour shears into.
    #[must_use]
    pub fn item(self) -> &'static str {
        match self {
            Self::White => "minecraft:white_wool",
            Self::Orange => "minecraft:orange_wool",
            Self::Magenta => "minecraft:magenta_wool",
            Self::LightBlue => "minecraft:light_blue_wool",
            Self::Yellow => "minecraft:yellow_wool",
            Self::Lime => "minecraft:lime_wool",
            Self::Pink => "minecraft:pink_wool",
            Self::Gray => "minecraft:gray_wool",
            Self::LightGray => "minecraft:light_gray_wool",
            Self::Cyan => "minecraft:cyan_wool",
            Self::Purple => "minecraft:purple_wool",
            Self::Blue => "minecraft:blue_wool",
            Self::Brown => "minecraft:brown_wool",
            Self::Green => "minecraft:green_wool",
            Self::Red => "minecraft:red_wool",
            Self::Black => "minecraft:black_wool",
        }
    }
}

/// What one mob in the working area is to the machine: the arm it answers, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anatomy {
    /// A grown, still-woolly `minecraft:sheep`: the shearing arm.
    Sheep {
        /// The colour it shears into.
        colour: Wool,
    },
    /// A `minecraft:squid`: the dye arm.
    Squid,
    /// A grown `minecraft:cow` or `minecraft:goat`: the milking arm.
    Milker,
    /// Anything the three arms do not reach.
    Inert,
}

/// `AnimalRancherTile.work()`'s first block: which arm, if any, this mob answers.
///
/// The jar asks `mob instanceof IShearable && isShearable(…)`, then `instanceof Squid`, then
/// `instanceof Animal`. The pin has no `Shearable` interface, so the two shearables this slice
/// reaches are named by type id: the sheep alone (`isShearable` is `!isSheared() && !isBaby()`).
/// `minecraft:cow`/`minecraft:goat` are `Animal`s whose bucket interaction yields milk, and that
/// interaction itself refuses a baby; every other id — the mooshroom, whose shearing upstream does
/// first, included — has no arm.
#[must_use]
pub fn anatomy(kind: &str, sheared: bool, colour: i32, baby: bool) -> Anatomy {
    match kind {
        "minecraft:sheep" if !sheared && !baby => Anatomy::Sheep {
            colour: Wool::from_colour(colour),
        },
        "minecraft:squid" => Anatomy::Squid,
        "minecraft:cow" | "minecraft:goat" if !baby => Anatomy::Milker,
        _ => Anatomy::Inert,
    }
}

/// The two random draws one attempt needs, as inputs: `nextInt(3)` for the wool count and the four
/// `nextBoolean()`s of the squid's gate reduced to what they decide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Draws {
    /// `level.random.nextInt(3)`: 0..=2, `1 +` this is the wool count.
    pub wool: i32,
    /// All four `nextBoolean()` draws, `1/16` of the time.
    pub dye: bool,
}

/// The attempt's outcome, as the glue needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// The bar is still counting down.
    Working,
    /// The bar reached zero and nothing was produced: no animal, no FE, a refused arm.
    Idle,
    /// Shear `count` wool of `colour` into the output, and leave the sheep sheared.
    Shear {
        /// The index into the attempt's `animals` of the sheep to shear.
        at: usize,
        /// The wool's colour.
        colour: Wool,
        /// How many wool one shearing gives, `1 + nextInt(3)`.
        count: i32,
    },
    /// One black dye into the output.
    Dye,
    /// `mb` mB of milk into the tank (the glue fills what the tank takes).
    Milk {
        /// The bucket's worth.
        mb: i32,
    },
}

/// The machine's numbers: the buffer, the bar and the tank.
#[derive(Debug, Clone, PartialEq)]
pub struct Rancher {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..=bar length`; the machine starts it full and it counts down.
    pub progress: i32,
    /// mB in the tank, whatever fluid it holds (a pipe may fill it, Titanium's default action is
    /// `BOTH`).
    pub tank_mb: i32,
    /// Whether the tank holds this machine's milk rather than a fluid a pipe put there.
    pub milk: bool,
}

impl Default for Rancher {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: MAX_PROGRESS,
            tank_mb: 0,
            milk: true,
        }
    }
}

impl Rancher {
    /// A machine from its saved compound; values outside the capacities are clamped, and a bar the
    /// save never wrote starts full (`ProgressBarComponent(maxProgress, maxProgress)`).
    #[must_use]
    pub fn from_saved(energy: i32, progress: Option<i32>, tank_mb: i32, milk: bool) -> Self {
        let tank_mb = tank_mb.clamp(0, TANK_MB);
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.unwrap_or(MAX_PROGRESS).clamp(0, MAX_PROGRESS),
            tank_mb,
            milk: milk || tank_mb == 0,
        }
    }

    /// The bar's length with the seated augments: `floor(100 × efficiency)`, never below 1.
    #[must_use]
    pub fn bar_length(augments: Augments) -> i32 {
        let scaled = (MAX_PROGRESS as f32 * augments.efficiency).floor() as i32;
        scaled.max(1)
    }

    /// The bar `amount` leaves behind: `(int)(maxProgress × amount)`, Java's float arithmetic.
    #[must_use]
    pub fn rest(bar: i32, amount: f32) -> i32 {
        (bar as f32 * amount).floor() as i32
    }

    /// The wool one shearing gives: `1 + nextInt(3)`, the draw bounded to the jar's three values.
    #[must_use]
    pub fn wool_count(draw: i32) -> i32 {
        1 + draw.clamp(0, WOOL_EXTRA - 1)
    }

    /// One server tick: fall the bar by this tick's step, and at zero run one attempt over
    /// `animals` in the level's order.
    ///
    /// `output_full` is `ItemStackUtils.isInventoryFull(output)` — the squid's gate.
    pub fn tick(
        &mut self,
        augments: Augments,
        animals: &[Anatomy],
        output_full: bool,
        draws: Draws,
    ) -> Step {
        let bar = Self::bar_length(augments);
        // `setProgress(progress - progressIncrease)` with the speed augment as the step: the bar
        // falls by its value, and may reach or pass zero.
        self.progress = (self.progress - augments.speed.max(1)).max(0);
        if self.progress > 0 {
            return Step::Working;
        }
        // `if (hasEnergy(powerPerOperation))` wraps the whole loop: an empty buffer does nothing,
        // not even shear a sheep it could not charge for.
        if self.energy < POWER_PER_OPERATION {
            self.progress = bar;
            return Step::Idle;
        }
        let chosen = animals
            .iter()
            .enumerate()
            .find_map(|(at, animal)| match *animal {
                Anatomy::Sheep { colour } => Some(Step::Shear {
                    at,
                    colour,
                    count: Self::wool_count(draws.wool),
                }),
                Anatomy::Squid => (!output_full && draws.dye).then_some(Step::Dye),
                Anatomy::Milker => {
                    (self.tank_mb + MILK_MB <= TANK_MB).then_some(Step::Milk { mb: MILK_MB })
                }
                Anatomy::Inert => None,
            });
        match chosen {
            Some(step) => {
                self.energy -= POWER_PER_OPERATION;
                self.progress = Self::rest(bar, WORK_AMOUNT);
                step
            }
            None => {
                self.progress = bar;
                Step::Idle
            }
        }
    }

    /// Put milk in the tank: `fillForced`'s clamp, and never over a fluid a pipe put there (a
    /// NeoForge tank only ever holds one fluid, so filling another one adds nothing).
    pub fn fill_milk(&mut self, mb: i32) -> i32 {
        if self.tank_mb > 0 && !self.milk {
            return 0;
        }
        let taken = mb.max(0).min(TANK_MB - self.tank_mb);
        if taken > 0 {
            self.milk = true;
            self.tank_mb += taken;
        }
        taken
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

    /// A rancher with `energy` FE and its bar one tick from the attempt.
    fn ready(energy: i32, tank_mb: i32) -> Rancher {
        Rancher {
            energy,
            progress: 1,
            tank_mb,
            ..Rancher::default()
        }
    }

    fn sheep() -> Anatomy {
        anatomy("minecraft:sheep", false, 0, false)
    }

    fn cow() -> Anatomy {
        anatomy("minecraft:cow", false, 0, false)
    }

    #[test]
    fn the_constants_are_the_config_s_own() {
        assert_eq!(MAX_PROGRESS, 100);
        assert_eq!(POWER_PER_OPERATION, 400);
        assert_eq!(CAPACITY_FE, 10_000);
        assert_eq!(TANK_MB, 8_000);
        assert_eq!(OUTPUT_SLOTS, 15);
        assert_eq!(MILK_MB, 1_000);
        assert_eq!(
            Rancher::rest(MAX_PROGRESS, WORK_AMOUNT),
            35,
            "floor(100 * 0.35)"
        );
        assert_eq!(Rancher::rest(MAX_PROGRESS, IDLE_AMOUNT), 100);
        assert_eq!(MILK, "industrialforegoing:milk");
    }

    #[test]
    fn a_cow_fills_the_tank_with_a_thousand_mb_for_four_hundred_fe() {
        let mut rancher = ready(10_000, 0);
        assert_eq!(
            rancher.tick(Augments::NONE, &[cow()], false, Draws::default()),
            Step::Milk { mb: 1_000 }
        );
        assert_eq!(rancher.energy, 9_600, "400 FE an operation");
        assert_eq!(rancher.progress, 35, "WorkAction(0.35f, 400)");
        assert_eq!(rancher.fill_milk(1_000), 1_000);
        assert_eq!(rancher.tank_mb, 1_000);
        assert!(rancher.milk);
    }

    #[test]
    fn the_milking_arm_needs_room_for_a_whole_bucket() {
        // `tank.getFluidAmount() + 1000 <= tank.getCapacity()`: 7,000 is the last allowed amount.
        let mut room = ready(10_000, 7_000);
        assert_eq!(
            room.tick(Augments::NONE, &[cow()], false, Draws::default()),
            Step::Milk { mb: 1_000 }
        );
        assert_eq!(room.energy, 9_600);
        let mut full = ready(10_000, 7_001);
        assert_eq!(
            full.tick(Augments::NONE, &[cow()], false, Draws::default()),
            Step::Idle
        );
        assert_eq!(full.energy, 10_000, "a refused arm spends nothing");
        assert_eq!(full.progress, MAX_PROGRESS, "and rests the bar full");
    }

    #[test]
    fn a_sheep_gives_one_two_or_three_wool_of_its_colour() {
        for (draw, want) in [(0, 1), (1, 2), (2, 3), (-5, 1), (99, 3)] {
            let mut rancher = ready(10_000, 0);
            assert_eq!(
                rancher.tick(
                    Augments::NONE,
                    &[anatomy("minecraft:sheep", false, 14, false)],
                    false,
                    Draws {
                        wool: draw,
                        dye: false
                    }
                ),
                Step::Shear {
                    at: 0,
                    colour: Wool::Red,
                    count: want
                },
                "nextInt(3) = {draw}"
            );
            assert_eq!(rancher.energy, 9_600);
        }
    }

    #[test]
    fn the_sixteen_colours_are_vanilla_s_dye_order() {
        let items: Vec<&str> = (0..16).map(|id| Wool::from_colour(id).item()).collect();
        assert_eq!(items[0], "minecraft:white_wool");
        assert_eq!(items[1], "minecraft:orange_wool");
        assert_eq!(items[2], "minecraft:magenta_wool");
        assert_eq!(items[3], "minecraft:light_blue_wool");
        assert_eq!(items[7], "minecraft:gray_wool");
        assert_eq!(items[8], "minecraft:light_gray_wool");
        assert_eq!(items[14], "minecraft:red_wool");
        assert_eq!(items[15], "minecraft:black_wool");
        assert_eq!(
            Wool::from_colour(-1),
            Wool::White,
            "a colour outside the enum reads as the fresh sheep's white"
        );
        assert_eq!(Wool::from_colour(99), Wool::White);
    }

    #[test]
    fn the_arms_are_the_ones_the_jar_names() {
        assert_eq!(
            sheep(),
            Anatomy::Sheep {
                colour: Wool::White
            }
        );
        assert_eq!(cow(), Anatomy::Milker);
        assert_eq!(
            anatomy("minecraft:goat", false, 0, false),
            Anatomy::Milker,
            "a goat's bucket interaction yields milk too"
        );
        assert_eq!(anatomy("minecraft:squid", false, 0, false), Anatomy::Squid);
        assert_eq!(
            anatomy("minecraft:sheep", false, 0, true),
            Anatomy::Inert,
            "isShearable is `!isSheared() && !isBaby()`"
        );
        assert_eq!(
            anatomy("minecraft:sheep", true, 0, false),
            Anatomy::Inert,
            "a bare sheep has nothing to shear"
        );
        assert_eq!(
            anatomy("minecraft:cow", false, 0, true),
            Anatomy::Inert,
            "a baby cow's bucket interaction is refused"
        );
        assert_eq!(
            anatomy("minecraft:mooshroom", false, 0, false),
            Anatomy::Inert,
            "the mooshroom's shearing is the arm upstream runs first, and this slice does not ship it"
        );
        assert_eq!(
            anatomy("minecraft:villager", false, 0, false),
            Anatomy::Inert,
            "not an Animal"
        );
    }

    #[test]
    fn the_arms_are_tried_in_the_jar_s_order() {
        // `IShearable` before `Squid` before `Animal`: the first arm that answers wins.
        let mut rancher = ready(10_000, 0);
        assert_eq!(
            rancher.tick(
                Augments::NONE,
                &[Anatomy::Inert, sheep(), cow()],
                false,
                Draws::default()
            ),
            Step::Shear {
                at: 1,
                colour: Wool::White,
                count: 1
            },
            "a sheep answers before the cow behind it"
        );
        let mut squid_first = ready(10_000, 0);
        assert_eq!(
            squid_first.tick(
                Augments::NONE,
                &[Anatomy::Squid, cow()],
                false,
                Draws {
                    wool: 0,
                    dye: false
                }
            ),
            Step::Milk { mb: 1_000 },
            "a squid whose four draws fail falls through to the next mob"
        );
    }

    #[test]
    fn the_squid_needs_a_free_slot_and_all_four_draws() {
        let mut yes = ready(10_000, 0);
        assert_eq!(
            yes.tick(
                Augments::NONE,
                &[Anatomy::Squid],
                false,
                Draws { wool: 0, dye: true }
            ),
            Step::Dye
        );
        assert_eq!(yes.energy, 9_600);
        assert_eq!(yes.progress, 35);
        let mut no_draw = ready(10_000, 0);
        assert_eq!(
            no_draw.tick(
                Augments::NONE,
                &[Anatomy::Squid],
                false,
                Draws {
                    wool: 0,
                    dye: false
                }
            ),
            Step::Idle
        );
        let mut full = ready(10_000, 0);
        assert_eq!(
            full.tick(
                Augments::NONE,
                &[Anatomy::Squid],
                true,
                Draws { wool: 0, dye: true }
            ),
            Step::Idle,
            "`!ItemStackUtils.isInventoryFull(output)`"
        );
        assert_eq!(full.energy, 10_000);
    }

    #[test]
    fn nothing_in_the_area_spends_nothing_and_rests_the_bar_full() {
        let mut rancher = ready(10_000, 0);
        assert_eq!(
            rancher.tick(Augments::NONE, &[], false, Draws::default()),
            Step::Idle
        );
        assert_eq!(rancher.energy, 10_000);
        assert_eq!(rancher.progress, MAX_PROGRESS);
        assert_eq!(rancher.tank_mb, 0);
    }

    #[test]
    fn without_four_hundred_fe_no_arm_runs() {
        let mut rancher = ready(POWER_PER_OPERATION - 1, 0);
        assert_eq!(
            rancher.tick(Augments::NONE, &[sheep()], false, Draws::default()),
            Step::Idle,
            "hasEnergy(400) wraps the whole loop"
        );
        assert_eq!(rancher.energy, 399);
        assert_eq!(rancher.progress, MAX_PROGRESS);
        let mut exact = ready(POWER_PER_OPERATION, 0);
        assert_eq!(
            exact.tick(Augments::NONE, &[sheep()], false, Draws::default()),
            Step::Shear {
                at: 0,
                colour: Wool::White,
                count: 1
            }
        );
        assert_eq!(exact.energy, 0);
    }

    #[test]
    fn the_bar_is_a_hundred_ticks_and_rests_at_thirty_five() {
        let mut rancher = Rancher::default();
        assert_eq!(
            rancher.tick(Augments::NONE, &[], false, Draws::default()),
            Step::Working
        );
        assert_eq!(rancher.progress, 99);
        let mut last = ready(10_000, 0);
        assert_eq!(
            last.tick(Augments::NONE, &[], false, Draws::default()),
            Step::Idle
        );
        assert_eq!(
            last.progress, 100,
            "an idle attempt rests the bar full again"
        );
    }

    #[test]
    fn an_efficiency_augment_shortens_the_bar_and_its_rest() {
        let augments = augments_of([None, Some(efficiency(0.8))]);
        assert_eq!(Rancher::bar_length(augments), 80, "floor(100 * 0.8)");
        assert_eq!(Rancher::rest(80, WORK_AMOUNT), 28, "floor(80 * 0.35)");
        let mut rancher = ready(10_000, 0);
        assert_eq!(
            rancher.tick(augments, &[cow()], false, Draws::default()),
            Step::Milk { mb: 1_000 }
        );
        assert_eq!(rancher.progress, 28);
    }

    #[test]
    fn a_speed_augment_falls_the_bar_four_a_tick() {
        let augments = augments_of([Some(speed(4.0)), None]);
        let mut rancher = Rancher {
            energy: 10_000,
            progress: 20,
            ..Rancher::default()
        };
        assert_eq!(
            rancher.tick(augments, &[], false, Draws::default()),
            Step::Working
        );
        assert_eq!(rancher.progress, 16, "4 a tick");
        assert_eq!(
            rancher.tick(augments, &[], false, Draws::default()),
            Step::Working
        );
        assert_eq!(rancher.progress, 12);
        // The bar may reach or pass zero, and the attempt runs on that tick.
        let mut nearly = Rancher {
            energy: 10_000,
            progress: 3,
            ..Rancher::default()
        };
        assert_eq!(
            nearly.tick(augments, &[cow()], false, Draws::default()),
            Step::Milk { mb: 1_000 }
        );
        assert_eq!(
            nearly.progress, 35,
            "a speed addon does not shorten the bar"
        );
    }

    #[test]
    fn the_tank_holds_milk_and_a_foreign_fluid_blocks_a_fill() {
        let mut milk = Rancher::from_saved(0, None, 7_500, true);
        assert_eq!(milk.fill_milk(1_000), 500, "fillForced clamps to the tank");
        assert_eq!(milk.tank_mb, TANK_MB);
        assert_eq!(milk.fill_milk(1_000), 0);
        let mut foreign = Rancher::from_saved(0, None, 500, false);
        assert_eq!(
            foreign.fill_milk(1_000),
            0,
            "a NeoForge tank holds one fluid, so milk over water adds nothing"
        );
        assert_eq!(foreign.tank_mb, 500);
        let mut empty = Rancher::from_saved(0, None, 0, false);
        assert_eq!(
            empty.fill_milk(1_000),
            1_000,
            "an empty tank takes the milk"
        );
        assert!(empty.milk);
    }

    #[test]
    fn a_bar_the_save_never_wrote_starts_full() {
        let mut rancher = Rancher::from_saved(0, None, 0, false);
        assert_eq!(rancher.progress, MAX_PROGRESS);
        assert_eq!(
            rancher.tick(Augments::NONE, &[], false, Draws::default()),
            Step::Working
        );
        assert_eq!(rancher.progress, MAX_PROGRESS - 1);
        let saved = Rancher::from_saved(90_000, Some(-3), 90_000, true);
        assert_eq!(saved.energy, CAPACITY_FE);
        assert_eq!(saved.progress, 0);
        assert_eq!(saved.tank_mb, TANK_MB);
    }
}
