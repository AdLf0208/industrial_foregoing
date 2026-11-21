//! Pure dissolution-chamber arithmetic for the Industrial Foregoing port.
//!
//! A second pure module beside [`crate::runtime`] (which stays byte-identical): the rules of
//! `DissolutionChamberTile` over Titanium's `IndustrialProcessingTile`, with no game access.
//! Upstream (`Industrial-Foregoing` `ce2852db`, Titanium `e53e869`, read for behaviour only):
//!
//! - [`CAPACITY_FE`] FE of buffer, [`POWER_PER_TICK`] FE taken on each tick the bar rises.
//! - The bar starts at 0 and rises while the machine can work (`shouldWork() && energy >=
//!   POWER_PER_TICK && canIncrease()`), one a tick, or the seated speed augment's value from the
//!   second tick on (Titanium sets `progressIncrease` after the first `setProgress`).
//! - At `maxProgress` the operation runs once: one item out of **every** input slot, the recipe's
//!   fluid drained, the output fluid filled and the product inserted; the bar restarts at 0.
//!   `maxProgress` is the recipe's `processingTime`, scaled by `floor(processingTime * efficiency)`
//!   when an efficiency augment is seated (Titanium's `onStart`).
//! - A bar that cannot work resets to 0 and takes no FE (`MultiProgressBarHandler.update`).
//! - A recipe matches only when the input slots hold **exactly** its item set (each ingredient
//!   claims a different slot, nothing is left over) and the input tank holds its fluid, at least
//!   its amount (`DissolutionChamberRecipe.matches`).
//!
//! The fluids are this mod's own (`industrialforegoing:latex`, `:pink_slime`, `:ether_gas`): they
//! are declared fluid keys and live in the block entity's own `fluids` compound, because the pin's
//! `native_fluid_insert` takes only water and lava.

use crate::runtime::{AUGMENT_SLOTS, Augment, Augments};

/// The recipe type every recipe file of this machine names (`ModuleCore.DISSOLUTION_TYPE`).
pub const RECIPE_TYPE: &str = "industrialforegoing:dissolution_chamber";
/// `DissolutionChamberConfig.maxStoredPower`.
pub const CAPACITY_FE: i32 = 10_000;
/// `DissolutionChamberConfig.powerPerTick`.
pub const POWER_PER_TICK: i32 = 90;
/// `DissolutionChamberConfig.maxInputTankSize`.
pub const INPUT_TANK_MB: i32 = 8_000;
/// `DissolutionChamberConfig.maxOutputTankSize`.
pub const OUTPUT_TANK_MB: i32 = 16_000;
/// `SidedInventoryComponent("input", …, 8, 0)`: the eight input slots, one item each.
pub const INPUT_SLOTS: usize = 8;
/// `SidedInventoryComponent("output", …, 3, 2)`: the three output slots.
pub const OUTPUT_SLOTS: usize = 3;
/// Every slot the container declares, inputs then outputs.
pub const SLOTS: usize = INPUT_SLOTS + OUTPUT_SLOTS;
/// `DissolutionChamberConfig.maxProgress`: the bar's fallback length when no recipe is in force.
pub const DEFAULT_TICKS: i32 = 100;

/// A fluid stack's identity in this machine: the fluid id and its amount in millibuckets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tank {
    /// The fluid id (`industrialforegoing:latex`); empty when the tank holds nothing.
    pub fluid: String,
    /// Millibuckets.
    pub mb: i32,
}

impl Tank {
    /// An empty tank.
    pub const EMPTY: Self = Self {
        fluid: String::new(),
        mb: 0,
    };

    /// A tank of `mb` of `fluid`.
    #[must_use]
    pub fn new(fluid: impl Into<String>, mb: i32) -> Self {
        let mb = mb.max(0);
        if mb == 0 {
            return Self::EMPTY;
        }
        Self {
            fluid: fluid.into(),
            mb,
        }
    }

    /// Whether the tank holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mb <= 0
    }

    /// Room left under `capacity`.
    #[must_use]
    pub fn room(&self, capacity: i32) -> i32 {
        (capacity - self.mb).max(0)
    }

    /// This tank with at most `capacity` in it.
    #[must_use]
    pub fn clamped(&self, capacity: i32) -> Self {
        if self.is_empty() {
            return Self::EMPTY;
        }
        Self {
            fluid: self.fluid.clone(),
            mb: self.mb.clamp(0, capacity),
        }
    }
}

/// One ingredient of a recipe: every item id it accepts (a tag, resolved by the game at decode
/// time). `DissolutionChamberRecipe.input` is `Ingredient.CODEC.listOf(0, 8)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingredient {
    /// The item ids one cell may hold.
    pub items: Vec<String>,
}

impl Ingredient {
    /// Whether `item` fits this ingredient.
    #[must_use]
    pub fn accepts(&self, item: &str) -> bool {
        self.items.iter().any(|candidate| candidate == item)
    }
}

/// A product: the item id and how many (`ItemStack.CODEC`'s `id` and `count`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    /// `minecraft:experience_bottle`, `industrialforegoing:machine_frame_simple`, …
    pub id: String,
    /// 1 through 99.
    pub count: u32,
}

/// One decoded `industrialforegoing:dissolution_chamber` recipe.
#[derive(Debug, Clone, PartialEq)]
pub struct ChamberRecipe {
    /// `industrialforegoing:dissolution_chamber/<name>`, as its file names it.
    pub id: String,
    /// The ingredients, in file order.
    pub input: Vec<Ingredient>,
    /// `inputFluid`: the fluid and its amount.
    pub fluid: Tank,
    /// `processingTime`, in ticks.
    pub ticks: i32,
    /// `output`; a recipe names one in every file this package carries.
    pub output: Option<Product>,
    /// `outputFluid`, when the recipe makes one.
    pub output_fluid: Option<Tank>,
}

/// What one tick did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Step {
    /// The machine could not work: the bar is at 0 and no FE was taken.
    #[default]
    Idle,
    /// The bar rose and [`POWER_PER_TICK`] FE was taken.
    Working,
    /// The operation ran: the glue consumes one of each input and inserts this.
    Completed(Completion),
}

/// What a finished operation hands the glue to write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completion {
    /// The stack to insert into the output slots.
    pub output: Option<Product>,
    /// The fluid to add to the output tank.
    pub output_fluid: Option<Tank>,
    /// The fluid the operation took out of the input tank.
    pub consumed_fluid: Tank,
}

/// The machine's saved state: the buffer, the bar, the two tanks and the bar's step.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    /// FE in the buffer.
    pub energy: i32,
    /// The bar, `0..max`; 0 is a bar that has not started.
    pub progress: i32,
    /// The input tank (`input_fluid`: 8,000 mB).
    pub input: Tank,
    /// The output tank (`output_fluid`: 16,000 mB).
    pub output: Tank,
    /// Titanium's `progressIncrease`: last tick's rate, 1 until a speed augment sets it.
    pub increase: i32,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            energy: 0,
            progress: 0,
            input: Tank::EMPTY,
            output: Tank::EMPTY,
            increase: 1,
        }
    }
}

impl Machine {
    /// A machine from its saved compound; values outside the capacities are clamped.
    #[must_use]
    pub fn from_saved(energy: i32, progress: i32, input: Tank, output: Tank) -> Self {
        Self {
            energy: energy.clamp(0, CAPACITY_FE),
            progress: progress.max(0),
            input: input.clamped(INPUT_TANK_MB),
            output: output.clamped(OUTPUT_TANK_MB),
            increase: 1,
        }
    }

    /// The bar's length for `recipe` with the seated augments: `processingTime`, scaled by
    /// `floor(processingTime * efficiency)` (Titanium's `onStart`), never below 1.
    #[must_use]
    pub fn bar_length(recipe: &ChamberRecipe, augments: Augments) -> i32 {
        // Java's `(int) Math.floor(getMaxProgress() * float)`: a float multiply, so
        // `300 * 0.8f` is 240.0000036 and floors to 240.
        let scaled = (recipe.ticks.max(1) as f32 * augments.efficiency).floor() as i32;
        scaled.max(1)
    }

    /// One server tick. `recipe` is the recipe the glue matched against the slots and the input
    /// tank; `accepts` says whether the output slots and tank can take its products.
    pub fn tick(
        &mut self,
        recipe: Option<&ChamberRecipe>,
        augments: Augments,
        accepts: bool,
    ) -> Step {
        // `MultiProgressBarHandler.update`: a bar that cannot work resets to 0 and takes nothing.
        let Some(recipe) = recipe else {
            self.progress = 0;
            return Step::Idle;
        };
        if !accepts || self.energy < POWER_PER_TICK {
            self.progress = 0;
            return Step::Idle;
        }
        let length = Self::bar_length(recipe, augments);
        // `ProgressBarComponent.tickBar`: the bar rises by `progressIncrease`, which
        // `onTickWork` sets *after* this tick, so a speed augment shows from the second tick.
        self.progress += self.increase.max(1);
        // `onTickWork`: the FE for this tick and the next tick's rate.
        self.energy -= POWER_PER_TICK;
        self.increase = augments.speed.max(1);
        if self.progress < length {
            return Step::Working;
        }
        // `tickBar` resets the bar, then `onFinishWork` runs the operation.
        self.progress = 0;
        let taken = recipe.fluid.mb.min(self.input.mb).max(0);
        self.input = if self.input.mb - taken > 0 {
            Tank::new(self.input.fluid.clone(), self.input.mb - taken)
        } else {
            Tank::EMPTY
        };
        if let Some(fluid) = &recipe.output_fluid {
            let room = self.output.room(OUTPUT_TANK_MB);
            let added = fluid.mb.min(room).max(0);
            if added > 0 {
                self.output = Tank::new(fluid.fluid.clone(), self.output.mb + added);
            }
        }
        Step::Completed(Completion {
            output: recipe.output.clone(),
            output_fluid: recipe.output_fluid.clone(),
            consumed_fluid: recipe.fluid.clone(),
        })
    }
}

/// The recipe in force for these input slots (`Option` per slot, the item id) and this input
/// tank: the first recipe of `recipes` whose ingredients each claim a different slot and whose
/// fluid is in the tank. Nothing is left over on a match (`DissolutionChamberRecipe.matches`).
#[must_use]
pub fn recipe_for<'a>(
    recipes: &'a [ChamberRecipe],
    slots: &[Option<String>],
    fluid: &Tank,
) -> Option<&'a ChamberRecipe> {
    recipe_index(recipes, slots, fluid).map(|index| &recipes[index])
}

/// [`recipe_for`]'s index into `recipes`, for a caller that keeps something beside each rule.
#[must_use]
pub fn recipe_index(
    recipes: &[ChamberRecipe],
    slots: &[Option<String>],
    fluid: &Tank,
) -> Option<usize> {
    recipes
        .iter()
        .position(|recipe| matches(recipe, slots, fluid))
}

/// `DissolutionChamberRecipe.matches`: every ingredient claims a different input slot, nothing
/// is left over, and the tank holds the recipe's fluid, at least its amount.
fn matches(recipe: &ChamberRecipe, slots: &[Option<String>], fluid: &Tank) -> bool {
    if fluid.fluid != recipe.fluid.fluid || fluid.mb < recipe.fluid.mb {
        return false;
    }
    let inputs = slots.len().min(INPUT_SLOTS);
    let mut claimed = vec![false; inputs];
    for ingredient in &recipe.input {
        let Some(index) = (0..inputs).find(|&slot| {
            !claimed[slot]
                && slots[slot]
                    .as_deref()
                    .is_some_and(|item| ingredient.accepts(item))
        }) else {
            return false;
        };
        claimed[index] = true;
    }
    // `handlerItems.isEmpty()`: every item that is there was claimed by an ingredient.
    (0..inputs).all(|slot| slots[slot].is_none() || claimed[slot])
}

/// The augments a block entity's two saved slots carry, as [`crate::runtime`] decodes them.
#[must_use]
pub fn augments_of(slots: [Option<Augment>; AUGMENT_SLOTS]) -> Augments {
    Augments::from_slots(slots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Augment, AugmentType};

    /// `industrialforegoing:dissolution_chamber/white_laser_lens`, as the jar writes it.
    fn lens() -> ChamberRecipe {
        ChamberRecipe {
            id: "industrialforegoing:dissolution_chamber/white_laser_lens".to_owned(),
            input: vec![
                Ingredient {
                    items: vec!["minecraft:glass_pane".to_owned()],
                };
                4
            ]
            .into_iter()
            .chain([Ingredient {
                items: vec!["minecraft:white_dye".to_owned()],
            }])
            .collect(),
            fluid: Tank::new("industrialforegoing:latex", 250),
            ticks: 100,
            output: Some(Product {
                id: "industrialforegoing:white_laser_lens".to_owned(),
                count: 1,
            }),
            output_fluid: None,
        }
    }

    /// `…/simple_machine_frame`: eight ingredients, two of them the same tag.
    fn simple_frame() -> ChamberRecipe {
        ChamberRecipe {
            id: "industrialforegoing:dissolution_chamber/simple_machine_frame".to_owned(),
            input: vec![
                Ingredient {
                    items: vec!["industrialforegoing:plastic".to_owned()],
                },
                Ingredient {
                    items: vec!["industrialforegoing:machine_frame_pity".to_owned()],
                },
                Ingredient {
                    items: vec!["industrialforegoing:plastic".to_owned()],
                },
                Ingredient {
                    items: vec!["minecraft:nether_brick".to_owned()],
                },
                Ingredient {
                    items: vec!["minecraft:nether_brick".to_owned()],
                },
                Ingredient {
                    items: vec!["minecraft:iron_ingot".to_owned()],
                },
                Ingredient {
                    items: vec!["industrialforegoing:gold_gear".to_owned()],
                },
                Ingredient {
                    items: vec!["minecraft:iron_ingot".to_owned()],
                },
            ],
            fluid: Tank::new("industrialforegoing:latex", 250),
            ticks: 300,
            output: Some(Product {
                id: "industrialforegoing:machine_frame_simple".to_owned(),
                count: 1,
            }),
            output_fluid: None,
        }
    }

    fn slots(items: &[&str]) -> Vec<Option<String>> {
        let mut out: Vec<Option<String>> =
            items.iter().map(|item| Some((*item).to_owned())).collect();
        out.resize(INPUT_SLOTS, None);
        out
    }

    fn latex() -> Tank {
        Tank::new("industrialforegoing:latex", 1_000)
    }

    // -----------------------------------------------------------------------
    // Matching (`DissolutionChamberRecipe.matches`)
    // -----------------------------------------------------------------------

    #[test]
    fn the_exact_item_set_matches() {
        let recipes = vec![lens()];
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
        ]);
        assert_eq!(
            recipe_for(&recipes, &held, &latex()).map(|r| r.ticks),
            Some(100)
        );
    }

    #[test]
    fn a_leftover_item_stops_the_match() {
        let recipes = vec![lens()];
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
            "minecraft:dirt",
        ]);
        assert_eq!(
            recipe_for(&recipes, &held, &latex()),
            None,
            "the chamber empties every input (handlerItems.isEmpty), so a sixth item blocks it"
        );
    }

    #[test]
    fn a_missing_item_stops_the_match() {
        let recipes = vec![lens()];
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
        ]);
        assert_eq!(recipe_for(&recipes, &held, &latex()), None);
    }

    #[test]
    fn two_copies_of_one_ingredient_need_two_items() {
        let recipes = vec![simple_frame()];
        let mut held = slots(&[
            "industrialforegoing:plastic",
            "industrialforegoing:machine_frame_pity",
            "minecraft:nether_brick",
            "minecraft:nether_brick",
            "minecraft:iron_ingot",
            "industrialforegoing:gold_gear",
            "minecraft:iron_ingot",
        ]);
        assert_eq!(
            recipe_for(&recipes, &held, &latex()),
            None,
            "one plastic cannot fill both #c:plastics cells"
        );
        held[7] = Some("industrialforegoing:plastic".to_owned());
        assert!(recipe_for(&recipes, &held, &latex()).is_some());
    }

    #[test]
    fn the_fluid_must_be_the_recipes_and_enough() {
        let recipes = vec![lens()];
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
        ]);
        assert_eq!(recipe_for(&recipes, &held, &Tank::EMPTY), None);
        assert_eq!(
            recipe_for(
                &recipes,
                &held,
                &Tank::new("industrialforegoing:latex", 249)
            ),
            None,
            "the recipe needs 250 mB"
        );
        assert_eq!(
            recipe_for(
                &recipes,
                &held,
                &Tank::new("industrialforegoing:pink_slime", 250)
            ),
            None,
            "another fluid is not this recipe's"
        );
        assert!(
            recipe_for(
                &recipes,
                &held,
                &Tank::new("industrialforegoing:latex", 250)
            )
            .is_some()
        );
    }

    #[test]
    fn a_recipe_with_no_items_matches_empty_slots() {
        let recipes = vec![ChamberRecipe {
            input: Vec::new(),
            output: None,
            ..lens()
        }];
        assert!(recipe_for(&recipes, &slots(&[]), &latex()).is_some());
        assert_eq!(
            recipe_for(&recipes, &slots(&["minecraft:dirt"]), &latex()),
            None
        );
    }

    #[test]
    fn the_first_matching_recipe_wins() {
        let first = lens();
        let mut second = lens();
        second.id = "industrialforegoing:dissolution_chamber/other".to_owned();
        let recipes = vec![first.clone(), second];
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
        ]);
        assert_eq!(
            recipe_index(&recipes, &held, &latex()),
            Some(0),
            "the first recipe in file order matches"
        );
        assert_eq!(
            recipe_for(&recipes, &held, &latex()).map(|r| r.id.clone()),
            Some(first.id)
        );
    }

    // -----------------------------------------------------------------------
    // The bar (`IndustrialProcessingTile` + `ProgressBarComponent`)
    // -----------------------------------------------------------------------

    /// A machine with a full buffer, its input tank full and a lens recipe ready.
    fn ready(recipe: &ChamberRecipe) -> (Machine, Vec<Option<String>>) {
        let mut machine = Machine::from_saved(
            CAPACITY_FE,
            0,
            Tank::new(recipe.fluid.fluid.clone(), INPUT_TANK_MB),
            Tank::EMPTY,
        );
        machine.increase = 1;
        let held = slots(&[
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:glass_pane",
            "minecraft:white_dye",
        ]);
        (machine, held)
    }

    #[test]
    fn a_hundred_tick_recipe_takes_a_hundred_ticks_and_nine_thousand_fe() {
        let recipe = lens();
        let (mut machine, _) = ready(&recipe);
        for tick in 1..100 {
            assert_eq!(
                machine.tick(Some(&recipe), Augments::NONE, true),
                Step::Working,
                "tick {tick} of a 100-tick recipe still works"
            );
            assert_eq!(machine.progress, tick);
        }
        assert_eq!(machine.energy, CAPACITY_FE - 99 * POWER_PER_TICK);
        let step = machine.tick(Some(&recipe), Augments::NONE, true);
        assert!(matches!(step, Step::Completed(_)), "{step:?}");
        assert_eq!(machine.energy, 1_000, "100 ticks at 90 FE each");
        assert_eq!(machine.progress, 0, "the bar restarts after the operation");
    }

    #[test]
    fn the_completion_carries_the_product_and_the_fluid_it_took() {
        let recipe = lens();
        let (mut machine, _) = ready(&recipe);
        let mut step = Step::Idle;
        for _ in 0..100 {
            step = machine.tick(Some(&recipe), Augments::NONE, true);
        }
        let Step::Completed(done) = step else {
            panic!("the 100th tick must finish the operation: {step:?}");
        };
        assert_eq!(
            done.output,
            Some(Product {
                id: "industrialforegoing:white_laser_lens".to_owned(),
                count: 1,
            })
        );
        assert_eq!(
            done.consumed_fluid,
            Tank::new("industrialforegoing:latex", 250)
        );
        assert_eq!(done.output_fluid, None);
    }

    #[test]
    fn the_output_fluid_is_passed_through() {
        let mut recipe = lens();
        recipe.output_fluid = Some(Tank::new("industrialforegoing:pink_slime", 300));
        let (mut machine, _) = ready(&recipe);
        let mut step = Step::Idle;
        for _ in 0..100 {
            step = machine.tick(Some(&recipe), Augments::NONE, true);
        }
        assert_eq!(
            step,
            Step::Completed(Completion {
                output: recipe.output.clone(),
                output_fluid: Some(Tank::new("industrialforegoing:pink_slime", 300)),
                consumed_fluid: Tank::new("industrialforegoing:latex", 250),
            })
        );
    }

    #[test]
    fn the_buffer_runs_out_mid_operation_and_the_bar_restarts() {
        let recipe = simple_frame();
        let (mut machine, _) = ready(&recipe);
        let mut worked = 0;
        for _ in 0..111 {
            assert_eq!(
                machine.tick(Some(&recipe), Augments::NONE, true),
                Step::Working
            );
            worked += 1;
        }
        assert_eq!(worked, 111, "10,000 FE buys 111 ticks at 90");
        assert_eq!(machine.energy, 10);
        assert_eq!(machine.progress, 111);
        assert_eq!(
            machine.tick(Some(&recipe), Augments::NONE, true),
            Step::Idle,
            "10 FE is not a tick's worth"
        );
        assert_eq!(machine.energy, 10, "an idle bar takes nothing");
        assert_eq!(machine.progress, 0, "a bar that cannot work resets");
    }

    #[test]
    fn a_blocked_output_holds_the_bar_and_takes_no_fe() {
        let recipe = lens();
        let (mut machine, _) = ready(&recipe);
        assert_eq!(
            machine.tick(Some(&recipe), Augments::NONE, false),
            Step::Idle
        );
        assert_eq!(machine.progress, 0);
        assert_eq!(
            machine.energy, CAPACITY_FE,
            "no FE is taken when it cannot work"
        );
    }

    #[test]
    fn the_bar_restarts_without_a_recipe() {
        let recipe = lens();
        let (mut machine, _) = ready(&recipe);
        for _ in 0..10 {
            machine.tick(Some(&recipe), Augments::NONE, true);
        }
        assert_eq!(machine.progress, 10);
        assert_eq!(machine.tick(None, Augments::NONE, true), Step::Idle);
        assert_eq!(machine.progress, 0);
        assert_eq!(machine.energy, CAPACITY_FE - 10 * POWER_PER_TICK);
    }

    #[test]
    fn a_nearly_finished_bar_needs_one_tick() {
        let recipe = simple_frame();
        let (mut machine, _) = ready(&recipe);
        machine.progress = Machine::bar_length(&recipe, Augments::NONE) - 1;
        assert!(matches!(
            machine.tick(Some(&recipe), Augments::NONE, true),
            Step::Completed(_)
        ));
    }

    // -----------------------------------------------------------------------
    // The augments (`titanium:augments`)
    // -----------------------------------------------------------------------

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

    #[test]
    fn an_efficiency_augment_shortens_the_run() {
        let recipe = simple_frame();
        let augments = augments_of([None, Some(efficiency(0.8))]);
        assert_eq!(
            Machine::bar_length(&recipe, augments),
            240,
            "floor(300 * 0.8), Titanium's onStart"
        );
        let (mut machine, _) = ready(&recipe);
        for tick in 1..240 {
            // A 240-tick run costs 21,600 FE, more than the 10,000 buffer: keep it fed, as a
            // cable would, so the run is the bar's length and not the buffer's.
            machine.energy = CAPACITY_FE;
            assert_eq!(machine.tick(Some(&recipe), augments, true), Step::Working);
            assert_eq!(machine.progress, tick);
        }
        machine.energy = CAPACITY_FE;
        assert!(matches!(
            machine.tick(Some(&recipe), augments, true),
            Step::Completed(_)
        ));
        assert_eq!(machine.energy, CAPACITY_FE - POWER_PER_TICK);
    }

    #[test]
    fn a_speed_augment_finishes_sooner_one_tick_late() {
        // Titanium raises the bar by the *previous* tick's rate: 1, then 4, 4, … so a
        // 100-tick bar is 1 + 4 k >= 100, i.e. the 26th tick.
        let recipe = lens();
        let augments = augments_of([Some(speed(4.0)), None]);
        let (mut machine, _) = ready(&recipe);
        let mut ticks = 0;
        loop {
            ticks += 1;
            if matches!(
                machine.tick(Some(&recipe), augments, true),
                Step::Completed(_)
            ) {
                break;
            }
            assert!(ticks < 100, "the bar never finished");
        }
        assert_eq!(ticks, 26);
        assert_eq!(machine.energy, CAPACITY_FE - 26 * POWER_PER_TICK);
    }

    #[test]
    fn a_speed_of_zero_still_advances_one_a_tick() {
        // `(int) 0.5f` is 0 in Java; the bar would never finish, so the port keeps the
        // first tick's own 1 and never lowers the rate below one.
        let recipe = lens();
        let augments = augments_of([Some(speed(0.5)), None]);
        let (mut machine, _) = ready(&recipe);
        for tick in 1..100 {
            machine.tick(Some(&recipe), augments, true);
            assert_eq!(machine.progress, tick);
        }
    }

    #[test]
    fn the_bar_length_is_the_recipes_without_an_efficiency_augment() {
        assert_eq!(Machine::bar_length(&lens(), Augments::NONE), 100);
        assert_eq!(Machine::bar_length(&simple_frame(), Augments::NONE), 300);
        let mut odd = lens();
        odd.ticks = 1;
        assert_eq!(Machine::bar_length(&odd, Augments::NONE), 1);
    }

    // -----------------------------------------------------------------------
    // The saved state
    // -----------------------------------------------------------------------

    #[test]
    fn from_saved_clamps_to_the_capacities() {
        let machine = Machine::from_saved(
            CAPACITY_FE + 5_000,
            3,
            Tank::new("industrialforegoing:latex", INPUT_TANK_MB + 1_000),
            Tank::new("industrialforegoing:pink_slime", OUTPUT_TANK_MB + 1_000),
        );
        assert_eq!(machine.energy, CAPACITY_FE);
        assert_eq!(machine.progress, 3);
        assert_eq!(machine.input.mb, INPUT_TANK_MB);
        assert_eq!(machine.output.mb, OUTPUT_TANK_MB);
        let empty = Machine::from_saved(-9, 0, Tank::EMPTY, Tank::EMPTY);
        assert_eq!(empty.energy, 0);
        assert!(empty.input.is_empty());
        assert!(empty.output.is_empty());
        assert_eq!(empty.increase, 1, "a fresh bar rises one a tick");
    }

    #[test]
    fn a_tank_of_zero_is_empty_and_keeps_no_name() {
        assert_eq!(Tank::new("industrialforegoing:latex", 0), Tank::EMPTY);
        assert_eq!(Tank::new("industrialforegoing:latex", -3), Tank::EMPTY);
        assert!(Tank::EMPTY.is_empty());
        assert_eq!(Tank::new("industrialforegoing:latex", 250).room(1_000), 750);
    }
}
