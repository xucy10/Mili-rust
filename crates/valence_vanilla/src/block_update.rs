use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_protocol::{BlockPos, BlockState, Direction};
use valence_server::ChunkLayer;

use crate::tick_schedule::TickScheduler;

/// All six cardinal directions for neighbor iteration.
const ALL_DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

/// Plugin that provides block update propagation.
pub struct BlockUpdatePlugin;

impl Plugin for BlockUpdatePlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BlockUpdateEvent>()
            .add_event::<NeighborUpdateEvent>()
            .add_systems(
                PostUpdate,
                process_neighbor_updates.before(crate::tick_schedule::process_scheduled_ticks),
            );
    }
}

/// Event fired when a block at a position changes state.
///
/// This is the primary event for block state changes. Listen to this
/// to react to blocks being placed, removed, or modified.
#[derive(Event, Copy, Clone, Debug)]
pub struct BlockUpdateEvent {
    /// The position of the block that was updated.
    pub position: BlockPos,
    /// The previous block state before the update.
    pub old_state: BlockState,
    /// The new block state after the update.
    pub new_state: BlockState,
}

/// Event fired to notify a neighbor that an adjacent block has changed.
///
/// This is sent for each of the 6 neighbors of a block that was updated.
/// Blocks like redstone, pistons, and observers listen to this event.
#[derive(Event, Copy, Clone, Debug)]
pub struct NeighborUpdateEvent {
    /// The position of the neighbor being notified.
    pub position: BlockPos,
    /// The position of the block that originally changed.
    pub source_position: BlockPos,
    /// The direction from the neighbor to the source block.
    pub direction: Direction,
    /// The previous block state of the source.
    pub source_old_state: BlockState,
    /// The new block state of the source.
    pub source_new_state: BlockState,
}

/// A scheduled tick entry for deferred block updates.
#[derive(Clone, Debug)]
pub struct ScheduledTick {
    /// The position of the block to tick.
    pub pos: BlockPos,
    /// The block state to apply when the tick fires.
    pub state: BlockState,
    /// The game tick at which this should execute.
    pub tick: u64,
    /// Priority for ordering when multiple ticks are due (lower = higher).
    pub priority: i32,
}

impl ScheduledTick {
    /// Creates a new scheduled tick.
    pub fn new(pos: BlockPos, state: BlockState, delay: u64, current_tick: u64) -> Self {
        Self {
            pos,
            state,
            tick: current_tick + delay,
            priority: 0,
        }
    }

    /// Creates a new scheduled tick with a specified priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Notifies all 6 neighbors of a block position about a change.
///
/// For each valid neighbor, a [`NeighborUpdateEvent`] is sent. This allows
/// systems to react to adjacent block changes (e.g., redstone, observers).
pub fn update_neighbors_at(
    pos: BlockPos,
    source_old_state: BlockState,
    source_new_state: BlockState,
    events: &mut EventWriter<NeighborUpdateEvent>,
) {
    for dir in ALL_DIRECTIONS {
        let neighbor_pos = pos.get_in_direction(dir);

        events.send(NeighborUpdateEvent {
            position: neighbor_pos,
            source_position: pos,
            direction: dir,
            source_old_state,
            source_new_state,
        });
    }
}

/// Sets a block at a position and triggers update propagation.
///
/// This function:
/// 1. Sets the block state on the chunk layer
/// 2. Sends a [`BlockUpdateEvent`] for the changed position
/// 3. Calls [`update_neighbors_at`] to notify all 6 neighbors
///
/// Returns the previous block state, or `None` if the position is invalid.
pub fn set_block_with_update(
    layer: &mut ChunkLayer,
    pos: BlockPos,
    new_state: BlockState,
    block_events: &mut EventWriter<BlockUpdateEvent>,
    neighbor_events: &mut EventWriter<NeighborUpdateEvent>,
) -> Option<BlockState> {
    let old_block = layer.block(pos)?;
    let old_state = old_block.state;

    if old_state == new_state {
        return Some(old_state);
    }

    layer.set_block(pos, new_state)?;

    block_events.send(BlockUpdateEvent {
        position: pos,
        old_state,
        new_state,
    });

    update_neighbors_at(pos, old_state, new_state, neighbor_events);

    Some(old_state)
}

/// Sets a block and schedules neighbor updates for the next tick.
///
/// Similar to [`set_block_with_update`], but defers the neighbor notification
/// by one tick via the [`TickScheduler`]. This is useful for blocks that
/// should not immediately propagate updates (e.g., pistons).
pub fn set_block_with_delayed_update(
    layer: &mut ChunkLayer,
    pos: BlockPos,
    new_state: BlockState,
    block_events: &mut EventWriter<BlockUpdateEvent>,
    scheduler: &mut ResMut<TickScheduler>,
) -> Option<BlockState> {
    let old_block = layer.block(pos)?;
    let old_state = old_block.state;

    if old_state == new_state {
        return Some(old_state);
    }

    layer.set_block(pos, new_state)?;

    block_events.send(BlockUpdateEvent {
        position: pos,
        old_state,
        new_state,
    });

    let tick = ScheduledTick::new(pos, new_state, 1, scheduler.current_tick());
    scheduler.schedule_tick(tick);

    Some(old_state)
}

/// Consumes `NeighborUpdateEvent`s that were not handled by any system.
///
/// This prevents "event was not consumed" warnings. Individual game mechanics
/// (redstone, pistons, observers) should have their own systems that read
/// `NeighborUpdateEvent` before this system runs.
fn process_neighbor_updates(mut events: EventReader<NeighborUpdateEvent>) {
    for _event in events.read() {
        // Events consumed but not acted upon.
    }
}

/// Schedules a tick for a block at a given position with a delay.
pub fn schedule_block_tick(
    scheduler: &mut ResMut<TickScheduler>,
    pos: BlockPos,
    state: BlockState,
    delay: u64,
) {
    let tick = ScheduledTick::new(pos, state, delay, scheduler.current_tick());
    scheduler.schedule_tick(tick);
}
