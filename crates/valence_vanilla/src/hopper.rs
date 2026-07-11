use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_inventory::Inventory;
use valence_protocol::{BlockPos, Direction, ItemStack};
use valence_server::layer::chunk::ChunkLayer;

/// Cooldown duration in ticks (20 ticks = 1 second).
const HOPPER_COOLDOWN: i32 = 8;

/// A hopper block that transfers items between inventories.
#[derive(Component, Debug, Clone)]
pub struct Hopper {
    /// Remaining cooldown ticks before the next transfer.
    pub transfer_cooldown: i32,
    /// Whether the hopper is enabled and can transfer items.
    pub enabled: bool,
    /// The direction items are pushed out (e.g., Down, North, South, East, West).
    /// If `None`, items are pushed down by default.
    pub output_direction: Direction,
    /// The block position of this hopper in the world.
    pub pos: BlockPos,
}

impl Default for Hopper {
    fn default() -> Self {
        Self {
            transfer_cooldown: 0,
            enabled: true,
            output_direction: Direction::Down,
            pos: BlockPos::new(0, 0, 0),
        }
    }
}

impl Hopper {
    /// Creates a new hopper with the specified output direction.
    pub fn new(output_direction: Direction, pos: BlockPos) -> Self {
        Self {
            transfer_cooldown: 0,
            enabled: true,
            output_direction,
            pos,
        }
    }

    /// Creates a hopper that outputs downward.
    pub fn downward(pos: BlockPos) -> Self {
        Self::new(Direction::Down, pos)
    }

    /// Check if the hopper can transfer items.
    pub fn can_transfer(&self) -> bool {
        self.enabled && self.transfer_cooldown <= 0
    }

    /// Reset the transfer cooldown.
    pub fn start_cooldown(&mut self) {
        self.transfer_cooldown = HOPPER_COOLDOWN;
    }

    /// Tick the cooldown down.
    pub fn tick_cooldown(&mut self) {
        if self.transfer_cooldown > 0 {
            self.transfer_cooldown -= 1;
        }
    }
}

/// Marker component for blocks that can receive items from hoppers
/// (e.g., chests, furnaces, other hoppers).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HopperReceiver;

/// Marker component for blocks that can provide items to hoppers
/// (e.g., chests, furnaces, other hoppers).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HopperProvider;

/// Plugin for the hopper system.
pub struct HopperPlugin;

impl Plugin for HopperPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, hopper_system);
    }
}

/// System that processes hopper item transfers every tick.
fn hopper_system(
    mut hoppers: Query<(Entity, &mut Hopper, &mut Inventory)>,
    mut inventories: Query<&mut Inventory, (Without<Hopper>, With<HopperReceiver>)>,
    chunk_layers: Query<&ChunkLayer>,
) {
    for (_entity, mut hopper, mut hopper_inv) in &mut hoppers {
        hopper.tick_cooldown();

        if !hopper.can_transfer() {
            continue;
        }

        let pos = hopper.pos;

        // Try to pull items from above first
        if try_pull_items(&mut hopper_inv, pos, &mut inventories, &chunk_layers) {
            hopper.start_cooldown();
            continue;
        }

        // Then try to push items to the output direction
        if try_push_items(
            &mut hopper_inv,
            pos,
            hopper.output_direction,
            &mut inventories,
            &chunk_layers,
        ) {
            hopper.start_cooldown();
        }
    }
}

/// Try to pull one item from the container above the hopper.
fn try_pull_items(
    hopper_inv: &mut Inventory,
    hopper_pos: BlockPos,
    inventories: &mut Query<&mut Inventory, (Without<Hopper>, With<HopperReceiver>)>,
    chunk_layers: &Query<&ChunkLayer>,
) -> bool {
    let above_pos = hopper_pos.get_in_direction(Direction::Up);

    // Try to find an entity-based inventory above
    if let Some(mut source_inv) = find_inventory_at_entity(above_pos, inventories) {
        return transfer_from_source_to_hopper(&mut source_inv, hopper_inv);
    }

    // Try to find a block-entity-based inventory above (e.g., a chest)
    if let Some(mut source_inv) = find_inventory_at_block(above_pos, chunk_layers) {
        return transfer_from_source_to_hopper(&mut source_inv, hopper_inv);
    }

    false
}

/// Try to push one item from the hopper to the output direction.
fn try_push_items(
    hopper_inv: &mut Inventory,
    hopper_pos: BlockPos,
    direction: Direction,
    inventories: &mut Query<&mut Inventory, (Without<Hopper>, With<HopperReceiver>)>,
    chunk_layers: &Query<&ChunkLayer>,
) -> bool {
    let target_pos = hopper_pos.get_in_direction(direction);

    // Try to find an entity-based inventory at the target
    if let Some(mut target_inv) = find_inventory_at_entity(target_pos, inventories) {
        return transfer_from_hopper_to_target(hopper_inv, &mut target_inv);
    }

    // Try to find a block-entity-based inventory at the target
    if let Some(mut target_inv) = find_inventory_at_block(target_pos, chunk_layers) {
        return transfer_from_hopper_to_target(hopper_inv, &mut target_inv);
    }

    false
}

/// Transfer items from a source inventory to the hopper.
fn transfer_from_source_to_hopper(source_inv: &mut Inventory, hopper_inv: &mut Inventory) -> bool {
    // Try to pull from the first slot of the source that has items
    for slot_idx in 0..source_inv.slot_count() {
        let source_item = source_inv.slot(slot_idx).clone();
        if source_item.is_empty() {
            continue;
        }

        let transfer_count = calculate_transfer_count(hopper_inv, &source_item);
        if transfer_count <= 0 {
            continue;
        }

        let dest_slot = find_matching_or_empty_slot(hopper_inv, &source_item);
        if let Some(dest_slot_idx) = dest_slot {
            let old_dest = hopper_inv.slot(dest_slot_idx).clone();

            if old_dest.is_empty() {
                let mut item_to_transfer = source_item.clone();
                item_to_transfer.count = transfer_count;
                hopper_inv.set_slot(dest_slot_idx, item_to_transfer);
            } else {
                let mut merged = old_dest;
                merged.count += transfer_count;
                hopper_inv.set_slot(dest_slot_idx, merged);
            }

            // Remove from source
            let remaining = source_item.count - transfer_count;
            if remaining <= 0 {
                source_inv.set_slot(slot_idx, ItemStack::EMPTY);
            } else {
                let mut remaining_item = source_item;
                remaining_item.count = remaining;
                source_inv.set_slot(slot_idx, remaining_item);
            }

            return true;
        }
    }
    false
}

/// Transfer items from the hopper to a target inventory.
fn transfer_from_hopper_to_target(hopper_inv: &mut Inventory, target_inv: &mut Inventory) -> bool {
    for slot_idx in 0..hopper_inv.slot_count() {
        let source_item = hopper_inv.slot(slot_idx).clone();
        if source_item.is_empty() {
            continue;
        }

        let transfer_count = calculate_transfer_count(target_inv, &source_item);
        if transfer_count <= 0 {
            continue;
        }

        let dest_slot = find_matching_or_empty_slot(target_inv, &source_item);
        if let Some(dest_slot_idx) = dest_slot {
            let old_dest = target_inv.slot(dest_slot_idx).clone();

            if old_dest.is_empty() {
                let mut item_to_transfer = source_item.clone();
                item_to_transfer.count = transfer_count;
                target_inv.set_slot(dest_slot_idx, item_to_transfer);
            } else {
                let mut merged = old_dest;
                merged.count += transfer_count;
                target_inv.set_slot(dest_slot_idx, merged);
            }

            let remaining = source_item.count - transfer_count;
            if remaining <= 0 {
                hopper_inv.set_slot(slot_idx, ItemStack::EMPTY);
            } else {
                let mut remaining_item = source_item;
                remaining_item.count = remaining;
                hopper_inv.set_slot(slot_idx, remaining_item);
            }

            return true;
        }
    }
    false
}

/// Calculate how many items can be transferred from source to destination.
fn calculate_transfer_count(dest_inv: &Inventory, source_item: &ItemStack) -> i8 {
    if source_item.is_empty() {
        return 0;
    }

    for slot_idx in 0..dest_inv.slot_count() {
        let dest_item = dest_inv.slot(slot_idx);
        if dest_item.is_empty() {
            return source_item.count.min(64);
        } else if dest_item.item == source_item.item && dest_item.count < 64 {
            let space = 64 - dest_item.count;
            return source_item.count.min(space);
        }
    }

    0
}

/// Find a matching slot or an empty slot in the inventory for the given item.
fn find_matching_or_empty_slot(inv: &Inventory, item: &ItemStack) -> Option<u16> {
    if item.is_empty() {
        return None;
    }

    // First try to find a matching slot with space
    for slot_idx in 0..inv.slot_count() {
        let slot_item = inv.slot(slot_idx);
        if slot_item.item == item.item && slot_item.count < 64 {
            return Some(slot_idx);
        }
    }

    // Then try to find an empty slot
    for slot_idx in 0..inv.slot_count() {
        if inv.slot(slot_idx).is_empty() {
            return Some(slot_idx);
        }
    }

    None
}

/// Find an entity-based inventory at the given block position.
fn find_inventory_at_entity(
    _pos: BlockPos,
    _inventories: &mut Query<&mut Inventory, (Without<Hopper>, With<HopperReceiver>)>,
) -> Option<Inventory> {
    // In a real implementation, you would:
    // 1. Query entities with Inventory and HopperReceiver at the given position
    // 2. Match entities by their BlockPos component
    // 3. Return the inventory
    //
    // This requires a spatial query system or position-based entity lookup,
    // which depends on your world architecture.
    None
}

/// Find a block-entity-based inventory at the given block position.
fn find_inventory_at_block(
    _pos: BlockPos,
    _chunk_layers: &Query<&ChunkLayer>,
) -> Option<Inventory> {
    // In a real implementation, you would:
    // 1. Check the block at the position (e.g., Chest, Barrel, Furnace)
    // 2. Look up the block entity's inventory from the chunk layer
    // 3. Return the inventory
    //
    // This requires block entity support in the chunk layer.
    None
}