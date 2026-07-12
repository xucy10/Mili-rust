use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use rand::Rng;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, ItemKind, ItemStack};
use valence_server::layer::chunk::ChunkLayer;

/// Crop types with their properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CropType {
    Wheat,
    Carrot,
    Potato,
    Beetroot,
    NetherWart,
}

impl CropType {
    /// Get the maximum age (growth stage) for this crop type.
    pub fn max_age(self) -> i32 {
        match self {
            CropType::Wheat | CropType::Carrot | CropType::Potato => 7,
            CropType::Beetroot | CropType::NetherWart => 3,
        }
    }

    /// Get the base growth chance (1 in N chance per tick).
    pub fn growth_chance(self) -> u32 {
        8
    }

    /// Check if this crop requires water nearby for growth.
    pub fn requires_water(self) -> bool {
        self != CropType::NetherWart
    }

    /// Check if this crop can be placed on the given block.
    pub fn can_place_on(self, block: BlockKind) -> bool {
        match self {
            CropType::Wheat | CropType::Carrot | CropType::Potato | CropType::Beetroot => {
                block == BlockKind::Farmland
            }
            CropType::NetherWart => block == BlockKind::SoulSand,
        }
    }

    /// Get the initial BlockState for this crop at age 0.
    pub fn initial_block_state(self) -> BlockState {
        match self {
            CropType::Wheat => BlockState::WHEAT,
            CropType::Carrot => BlockState::CARROTS,
            CropType::Potato => BlockState::POTATOES,
            CropType::Beetroot => BlockState::BEETROOTS,
            CropType::NetherWart => BlockState::NETHER_WART,
        }
    }
}

/// Component for a crop entity that can grow over time.
#[derive(Component, Debug, Clone)]
pub struct Crop {
    /// The type of crop.
    pub crop_type: CropType,
    /// Current growth stage (age).
    pub age: i32,
    /// Maximum growth stage.
    pub max_age: i32,
    /// Base growth chance (1 in N per tick).
    pub growth_chance: u32,
    /// The block position of this crop in the world.
    pub pos: BlockPos,
}

impl Crop {
    /// Creates a new crop of the specified type.
    pub fn new(crop_type: CropType, pos: BlockPos) -> Self {
        let max_age = crop_type.max_age();
        let growth_chance = crop_type.growth_chance();

        Self {
            crop_type,
            age: 0,
            max_age,
            growth_chance,
            pos,
        }
    }

    /// Check if the crop is fully grown.
    pub fn is_fully_grown(&self) -> bool {
        self.age >= self.max_age
    }

    /// Advance the crop to the next growth stage.
    pub fn grow(&mut self) {
        if !self.is_fully_grown() {
            self.age += 1;
        }
    }

    /// Apply bone meal to instantly grow the crop (advances by 2 stages).
    pub fn apply_bone_meal(&mut self) -> bool {
        if self.is_fully_grown() {
            return false;
        }

        let advancement = if self.age + 2 <= self.max_age {
            2
        } else {
            self.max_age - self.age
        };

        self.age += advancement;
        true
    }

    /// Get the BlockState corresponding to the current age.
    pub fn to_block_state(&self) -> BlockState {
        let age_value = match self.age {
            0 => PropValue::_0,
            1 => PropValue::_1,
            2 => PropValue::_2,
            3 => PropValue::_3,
            4 => PropValue::_4,
            5 => PropValue::_5,
            6 => PropValue::_6,
            _ => PropValue::_7,
        };
        self.crop_type
            .initial_block_state()
            .set(PropName::Age, age_value)
    }
}

/// Plugin for the crop growth system.
pub struct CropPlugin;

impl Plugin for CropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (crop_growth_system, bone_meal_growth_system));
    }
}

/// System that handles random crop growth ticks.
fn crop_growth_system(mut crops: Query<&mut Crop>, mut chunk_layers: Query<&mut ChunkLayer>) {
    let mut rng = rand::thread_rng();

    for mut crop in &mut crops {
        if crop.is_fully_grown() {
            continue;
        }

        let growth_check = rng.gen_range(0..crop.growth_chance);
        if growth_check != 0 {
            continue;
        }

        let pos = crop.pos;
        let light_level = get_light_level(pos, &chunk_layers);
        let has_water = check_nearby_water(pos, &chunk_layers);

        if check_growth_conditions(crop.crop_type, light_level, has_water) {
            crop.grow();
            set_crop_block_state(&crop, pos, &mut chunk_layers);
        }
    }
}

/// System that handles bone-mealed crop growth (applied instantly).
fn bone_meal_growth_system(mut crops: Query<&mut Crop>, mut chunk_layers: Query<&mut ChunkLayer>) {
    for mut crop in &mut crops {
        if crop.is_fully_grown() {
            continue;
        }

        let old_age = crop.age;
        crop.grow();
        crop.grow();
        if crop.age != old_age {
            set_crop_block_state(&crop, crop.pos, &mut chunk_layers);
        }
    }
}

/// Check if growth conditions are met for a crop.
pub fn check_growth_conditions(crop_type: CropType, light_level: i32, has_water: bool) -> bool {
    let min_light = match crop_type {
        CropType::NetherWart => 0,
        _ => 9,
    };

    if light_level < min_light {
        return false;
    }

    if crop_type.requires_water() && !has_water {
        return false;
    }

    true
}

/// Get the light level at a given position.
fn get_light_level(pos: BlockPos, chunk_layers: &Query<&ChunkLayer>) -> i32 {
    if let Ok(chunk_layer) = chunk_layers.get_single() {
        let above_pos = BlockPos::new(pos.x, pos.y + 1, pos.z);
        if let Some(above_ref) = chunk_layer.block(above_pos) {
            if above_ref.state.to_kind() == BlockKind::Air {
                return 15;
            }
        }
        return 10;
    }
    12
}

/// Check if there's water nearby (within 4 blocks horizontally, same or +1 Y).
fn check_nearby_water(pos: BlockPos, chunk_layers: &Query<&ChunkLayer>) -> bool {
    let Ok(chunk_layer) = chunk_layers.get_single() else {
        return true;
    };

    for dx in -4..=4 {
        for dz in -4..=4 {
            let check_pos = BlockPos::new(pos.x + dx, pos.y, pos.z + dz);
            if let Some(block_ref) = chunk_layer.block(check_pos) {
                if block_ref.state.to_kind() == BlockKind::Water {
                    return true;
                }
            }
            let above_pos = BlockPos::new(pos.x + dx, pos.y + 1, pos.z + dz);
            if let Some(block_ref) = chunk_layer.block(above_pos) {
                if block_ref.state.to_kind() == BlockKind::Water {
                    return true;
                }
            }
        }
    }
    false
}

/// Set the crop block state in the world based on current age.
fn set_crop_block_state(crop: &Crop, pos: BlockPos, chunk_layers: &mut Query<&mut ChunkLayer>) {
    let Ok(mut chunk_layer) = chunk_layers.get_single_mut() else {
        return;
    };
    let new_state = crop.to_block_state();
    chunk_layer.set_block(pos, new_state);
}

/// Get the item produced when a crop is harvested.
pub fn get_harvest_item(crop_type: CropType) -> ItemStack {
    match crop_type {
        CropType::Wheat => ItemStack::new(ItemKind::Wheat, 1, None),
        CropType::Carrot => ItemStack::new(ItemKind::Carrot, 1, None),
        CropType::Potato => ItemStack::new(ItemKind::Potato, 1, None),
        CropType::Beetroot => ItemStack::new(ItemKind::BeetrootSeeds, 1, None),
        CropType::NetherWart => ItemStack::new(ItemKind::NetherWart, 1, None),
    }
}

/// Get additional drops for crops (e.g., wheat seeds from wheat).
pub fn get_additional_drops(crop_type: CropType) -> Vec<ItemStack> {
    let mut drops = Vec::new();
    let mut rng = rand::thread_rng();

    match crop_type {
        CropType::Wheat => {
            let seed_count = rng.gen_range(0..4);
            if seed_count > 0 {
                drops.push(ItemStack::new(ItemKind::WheatSeeds, seed_count, None));
            }
        }
        CropType::Carrot => {
            let drop_count = rng.gen_range(1..5);
            if drop_count > 1 {
                drops.push(ItemStack::new(ItemKind::Carrot, drop_count - 1, None));
            }
        }
        CropType::Potato => {
            let drop_count = rng.gen_range(1..5);
            if drop_count > 1 {
                drops.push(ItemStack::new(ItemKind::Potato, drop_count - 1, None));
            }
        }
        CropType::Beetroot => {
            let seed_count = rng.gen_range(1..4);
            if seed_count > 0 {
                drops.push(ItemStack::new(ItemKind::BeetrootSeeds, seed_count, None));
            }
        }
        CropType::NetherWart => {
            let drop_count = rng.gen_range(2..5);
            if drop_count > 1 {
                drops.push(ItemStack::new(ItemKind::NetherWart, drop_count - 1, None));
            }
        }
    }

    drops
}
