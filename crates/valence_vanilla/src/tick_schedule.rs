use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use rand::Rng;
use valence_protocol::{BlockPos, BlockState};
use valence_server::ChunkLayer;

use crate::block_update::{NeighborUpdateEvent, ScheduledTick};

/// Plugin that provides tick scheduling and random tick processing.
pub struct TickSchedulePlugin;

impl Plugin for TickSchedulePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TickScheduler>()
            .add_event::<RandomTickEvent>()
            .add_event::<ScheduledTickEvent>()
            .add_systems(
                PostUpdate,
                (
                    increment_tick,
                    process_scheduled_ticks,
                    process_random_ticks,
                )
                    .chain(),
            );
    }
}

/// Resource that manages the game tick counter and scheduled tick queue.
///
/// The scheduler maintains a queue of [`ScheduledTick`]s that will be
/// executed when the game tick reaches their target tick.
#[derive(Resource)]
pub struct TickScheduler {
    /// Pending scheduled ticks, kept sorted by tick number.
    scheduled_ticks: Vec<ScheduledTick>,
    /// The current game tick counter.
    current_tick: u64,
    /// Number of random block ticks per section per game tick.
    /// Set to 0 to disable random ticks. Default is 3 (vanilla).
    random_tick_speed: u32,
}

impl Default for TickScheduler {
    fn default() -> Self {
        Self {
            scheduled_ticks: Vec::new(),
            current_tick: 0,
            random_tick_speed: 3,
        }
    }
}

impl TickScheduler {
    /// Creates a new tick scheduler with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a tick scheduler with a custom random tick speed.
    pub fn with_random_tick_speed(random_tick_speed: u32) -> Self {
        Self {
            random_tick_speed,
            ..Default::default()
        }
    }

    /// Returns the current game tick.
    pub fn current_tick(&self) -> u64 {
        self.current_tick
    }

    /// Returns the random tick speed (blocks per section per tick).
    pub fn random_tick_speed(&self) -> u32 {
        self.random_tick_speed
    }

    /// Sets the random tick speed.
    pub fn set_random_tick_speed(&mut self, speed: u32) {
        self.random_tick_speed = speed;
    }

    /// Schedules a tick to be executed in the future.
    pub fn schedule_tick(&mut self, tick: ScheduledTick) {
        self.scheduled_ticks.push(tick);
    }

    /// Returns `true` if there are ticks pending execution.
    pub fn has_pending_ticks(&self) -> bool {
        !self.scheduled_ticks.is_empty()
    }

    /// Returns the number of pending scheduled ticks.
    pub fn pending_tick_count(&self) -> usize {
        self.scheduled_ticks.len()
    }

    /// Drains all scheduled ticks that are due at or before the current tick.
    fn drain_due_ticks(&mut self) -> Vec<ScheduledTick> {
        let current = self.current_tick;
        let mut due = Vec::new();
        self.scheduled_ticks.retain(|t| {
            if t.tick <= current {
                due.push(ScheduledTick {
                    pos: t.pos,
                    state: t.state,
                    tick: t.tick,
                    priority: t.priority,
                });
                false
            } else {
                true
            }
        });
        // Sort by priority (lower = higher priority) then by tick
        due.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.tick.cmp(&b.tick)));
        due
    }
}

/// Event fired when a scheduled tick is executed.
#[derive(Event, Copy, Clone, Debug)]
pub struct ScheduledTickEvent {
    /// The position of the block that was ticked.
    pub position: BlockPos,
    /// The block state that was applied.
    pub state: BlockState,
    /// The game tick at which this was scheduled to fire.
    pub scheduled_tick: u64,
}

/// Event fired when a block receives a random tick.
///
/// Random ticks occur for random positions within each chunk section.
/// Blocks like crops, leaves, fire, and ice listen to this event.
#[derive(Event, Copy, Clone, Debug)]
pub struct RandomTickEvent {
    /// The position of the block that received the random tick.
    pub position: BlockPos,
    /// The current block state at the position.
    pub state: BlockState,
}

/// Increments the game tick counter each frame.
fn increment_tick(mut scheduler: ResMut<TickScheduler>) {
    scheduler.current_tick += 1;
}

/// Processes all scheduled ticks that are due at the current game tick.
///
/// Due ticks are drained from the scheduler and their block states are
/// applied to the world. A [`ScheduledTickEvent`] is sent for each
/// executed tick.
pub fn process_scheduled_ticks(
    mut scheduler: ResMut<TickScheduler>,
    mut layers: Query<&mut ChunkLayer>,
    mut tick_events: EventWriter<ScheduledTickEvent>,
    mut neighbor_events: EventWriter<NeighborUpdateEvent>,
) {
    let due_ticks = scheduler.drain_due_ticks();

    if due_ticks.is_empty() {
        return;
    }

    for layer in &mut layers {
        let layer = layer.into_inner();

        for tick in &due_ticks {
            // set_block returns the old block if the position was valid.
            if let Some(old_block) = layer.set_block(tick.pos, tick.state) {
                let old_state = old_block.state;
                if old_state != tick.state {
                    // Notify neighbors of the scheduled change
                    crate::block_update::update_neighbors_at(
                        tick.pos,
                        old_state,
                        tick.state,
                        &mut neighbor_events,
                    );
                }
            }

            tick_events.send(ScheduledTickEvent {
                position: tick.pos,
                state: tick.state,
                scheduled_tick: tick.tick,
            });
        }
    }
}

/// Processes random ticks for all loaded chunks.
///
/// For each chunk section, up to `random_tick_speed` random block positions
/// are selected. If a non-air block exists at that position, a
/// [`RandomTickEvent`] is sent.
///
/// Random ticks are used for natural processes like:
/// - Crop growth
/// - Leaf decay
/// - Fire spread
/// - Ice and snow formation
/// - Grass and flower spreading
fn process_random_ticks(
    scheduler: Res<TickScheduler>,
    layers: Query<&ChunkLayer>,
    mut random_events: EventWriter<RandomTickEvent>,
) {
    let speed = scheduler.random_tick_speed;
    if speed == 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for layer in &layers {
        let min_y = layer.min_y();
        let world_height = layer.height();
        let sections = world_height / 16;

        for (chunk_pos, _) in layer.chunks() {
            for sect_idx in 0..sections {
                let sect_min_y = min_y + sect_idx as i32 * 16;

                for _ in 0..speed {
                    let local_x = rng.gen_range(0_u32..16);
                    let local_y = rng.gen_range(0_u32..16);
                    let local_z = rng.gen_range(0_u32..16);

                    let global_x = chunk_pos.x * 16 + local_x as i32;
                    let global_y = sect_min_y + local_y as i32;
                    let global_z = chunk_pos.z * 16 + local_z as i32;

                    let pos = BlockPos::new(global_x, global_y, global_z);

                    if let Some(block_ref) = layer.block(pos) {
                        if !block_ref.state.is_air() {
                            random_events.send(RandomTickEvent {
                                position: pos,
                                state: block_ref.state,
                            });
                        }
                    }
                }
            }
        }
    }
}