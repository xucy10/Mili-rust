use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_inventory::Inventory;
use valence_protocol::{ItemKind, ItemStack};

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftingRegistry>()
            .add_systems(PreStartup, init_vanilla_recipes)
            .add_systems(Update, process_crafting);
    }
}

fn init_vanilla_recipes(mut registry: ResMut<CraftingRegistry>) {
    register_vanilla_recipes(&mut registry);
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct CraftingInput {
    pub slots: [Option<(ItemKind, i8)>; 9],
}

impl CraftingInput {
    pub fn from_inventory(inv: &Inventory, start_slot: u16) -> Self {
        let mut slots = [None; 9];
        for i in 0..9 {
            let item = inv.slot(start_slot + i as u16);
            if !item.is_empty() {
                slots[i] = Some((item.item, item.count));
            }
        }
        Self { slots }
    }

    pub fn normalized(&self) -> Self {
        let mut min_row = 3;
        let mut min_col = 3;
        let mut max_row = 0;
        let mut max_col = 0;

        for row in 0..3 {
            for col in 0..3 {
                if self.slots[row * 3 + col].is_some() {
                    min_row = min_row.min(row);
                    min_col = min_col.min(col);
                    max_row = max_row.max(row);
                    max_col = max_col.max(col);
                }
            }
        }

        if min_row > max_row {
            return Self { slots: [None; 9] };
        }

        let mut normalized = [None; 9];
        for row in min_row..=max_row {
            for col in min_col..=max_col {
                let src_idx = row * 3 + col;
                let dst_row = row - min_row;
                let dst_col = col - min_col;
                let dst_idx = dst_row * 3 + dst_col;
                normalized[dst_idx] = self.slots[src_idx];
            }
        }

        Self { slots: normalized }
    }

    pub fn width(&self) -> usize {
        let mut max_col = 0;
        for row in 0..3 {
            for col in 0..3 {
                if self.slots[row * 3 + col].is_some() {
                    max_col = max_col.max(col + 1);
                }
            }
        }
        max_col
    }

    pub fn height(&self) -> usize {
        let mut max_row = 0;
        for row in 0..3 {
            for col in 0..3 {
                if self.slots[row * 3 + col].is_some() {
                    max_row = max_row.max(row + 1);
                }
            }
        }
        max_row
    }
}

#[derive(Clone, Debug)]
pub struct CraftingRecipe {
    pub pattern: [Option<ItemKind>; 9],
    pub result: ItemStack,
    pub category: RecipeCategory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeCategory {
    Building,
    Redstone,
    Equipment,
    Misc,
}

impl CraftingRecipe {
    pub fn shaped(
        pattern: [Option<ItemKind>; 9],
        result: ItemStack,
        category: RecipeCategory,
    ) -> Self {
        Self {
            pattern,
            result,
            category,
        }
    }

    pub fn shapeless(
        ingredients: Vec<ItemKind>,
        result: ItemStack,
        category: RecipeCategory,
    ) -> Self {
        let mut pattern = [None; 9];
        for (i, item) in ingredients.into_iter().enumerate() {
            if i < 9 {
                pattern[i] = Some(item);
            }
        }
        Self {
            pattern,
            result,
            category,
        }
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        let normalized = input.normalized();
        for i in 0..9 {
            match (self.pattern[i], normalized.slots[i]) {
                (Some(a), Some((b, _))) if a == b => {}
                (None, None) => {}
                _ => return false,
            }
        }
        true
    }
}

#[derive(Resource, Default)]
pub struct CraftingRegistry {
    recipes: Vec<CraftingRecipe>,
}

impl CraftingRegistry {
    pub fn register(&mut self, recipe: CraftingRecipe) {
        self.recipes.push(recipe);
    }

    pub fn find_match(&self, input: &CraftingInput) -> Option<&CraftingRecipe> {
        self.recipes.iter().find(|r| r.matches(input))
    }

    pub fn recipes(&self) -> &[CraftingRecipe] {
        &self.recipes
    }
}

pub fn register_vanilla_recipes(registry: &mut CraftingRegistry) {
    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::Stick, 4, None),
        RecipeCategory::Misc,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::CraftingTable, 1, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            None,
            Some(ItemKind::Stick),
            None,
            None,
            Some(ItemKind::Stick),
            None,
        ],
        ItemStack::new(ItemKind::StoneAxe, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            None,
            Some(ItemKind::Cobblestone),
            None,
            None,
            Some(ItemKind::Stick),
            None,
            None,
            Some(ItemKind::Stick),
            None,
        ],
        ItemStack::new(ItemKind::StoneShovel, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::StoneSlab, 6, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::Stick),
            None,
            None,
            Some(ItemKind::Stick),
            None,
        ],
        ItemStack::new(ItemKind::IronPickaxe, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            None,
            None,
            Some(ItemKind::IronIngot),
            None,
            None,
            Some(ItemKind::Stick),
            Some(ItemKind::Stick),
            None,
        ],
        ItemStack::new(ItemKind::IronSword, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
        ],
        ItemStack::new(ItemKind::IronChestplate, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::IronHelmet, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
        ],
        ItemStack::new(ItemKind::IronLeggings, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::IronBoots, 1, None),
        RecipeCategory::Equipment,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
        ],
        ItemStack::new(ItemKind::Chest, 1, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            Some(ItemKind::IronIngot),
            None,
            Some(ItemKind::IronIngot),
            None,
        ],
        ItemStack::new(ItemKind::Bucket, 1, None),
        RecipeCategory::Misc,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            None,
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
            Some(ItemKind::Cobblestone),
        ],
        ItemStack::new(ItemKind::Furnace, 1, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
            Some(ItemKind::Redstone),
        ],
        ItemStack::new(ItemKind::RedstoneLamp, 1, None),
        RecipeCategory::Redstone,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            None,
            Some(ItemKind::Redstone),
            None,
            Some(ItemKind::Redstone),
            Some(ItemKind::Stick),
            Some(ItemKind::Redstone),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
        ],
        ItemStack::new(ItemKind::Repeater, 1, None),
        RecipeCategory::Redstone,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            None,
            Some(ItemKind::RedstoneTorch),
            None,
            Some(ItemKind::RedstoneTorch),
            Some(ItemKind::Quartz),
            Some(ItemKind::RedstoneTorch),
            Some(ItemKind::Stone),
            Some(ItemKind::Stone),
            Some(ItemKind::Stone),
        ],
        ItemStack::new(ItemKind::Comparator, 1, None),
        RecipeCategory::Redstone,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            None,
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            Some(ItemKind::OakPlanks),
        ],
        ItemStack::new(ItemKind::OakBoat, 1, None),
        RecipeCategory::Misc,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::OakDoor, 3, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::OakTrapdoor, 2, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shaped(
        [
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            Some(ItemKind::OakPlanks),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        ItemStack::new(ItemKind::OakSlab, 6, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shapeless(
        vec![ItemKind::Wheat],
        ItemStack::new(ItemKind::Bread, 1, None),
        RecipeCategory::Misc,
    ));

    registry.register(CraftingRecipe::shapeless(
        vec![
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
            ItemKind::Cobblestone,
        ],
        ItemStack::new(ItemKind::Furnace, 1, None),
        RecipeCategory::Building,
    ));

    registry.register(CraftingRecipe::shapeless(
        vec![ItemKind::GoldNugget; 9],
        ItemStack::new(ItemKind::GoldIngot, 1, None),
        RecipeCategory::Misc,
    ));
}

#[derive(Component)]
pub struct CraftingOutput {
    pub result: Option<ItemStack>,
}

impl Default for CraftingOutput {
    fn default() -> Self {
        Self { result: None }
    }
}

fn process_crafting(
    registry: Res<CraftingRegistry>,
    mut query: Query<(&Inventory, &mut CraftingOutput), Changed<Inventory>>,
) {
    for (inventory, mut output) in &mut query {
        let input = CraftingInput::from_inventory(inventory, 1);
        if let Some(recipe) = registry.find_match(&input) {
            output.result = Some(recipe.result.clone());
        } else {
            output.result = None;
        }
    }
}