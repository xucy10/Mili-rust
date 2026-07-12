pub mod comparator;
pub mod lamp;
pub mod piston;
pub mod repeater;
pub mod signal;
pub mod torch;
pub mod wire;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};
use valence_server::layer::chunk::ChunkLayer;

use comparator::RedstoneComparator;
use piston::Piston;
use repeater::RedstoneRepeater;
use signal::{
    get_horizontal_directions, get_power_level, is_redstone_conductor, offset_pos,
    RedstoneStrength, RedstoneUpdateEntry, RedstoneUpdateQueue, UpdateType, MAX_SIGNAL,
};
use torch::RedstoneTorch;

pub struct RedstonePlugin;

impl Plugin for RedstonePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RedstoneUpdateQueue>()
            .init_resource::<RedstoneSystemState>()
            .add_systems(Update, (update_redstone_components,));
    }
}

#[derive(Resource, Default)]
pub struct RedstoneSystemState {
    pub current_tick: u64,
}

fn get_block_state(chunk_layer: &ChunkLayer, pos: BlockPos) -> Option<BlockState> {
    chunk_layer.block(pos).map(|b| b.state)
}

fn update_redstone_components(
    mut chunk_layers: Query<&mut ChunkLayer>,
    mut update_queue: ResMut<RedstoneUpdateQueue>,
    mut system_state: ResMut<RedstoneSystemState>,
) {
    system_state.current_tick += 1;

    let positions: Vec<BlockPos> = update_queue.iter().map(|e| e.pos).collect();
    update_queue.clear();

    for mut chunk_layer in &mut chunk_layers {
        for pos in positions.iter().copied() {
            let state = match get_block_state(&chunk_layer, pos) {
                Some(s) => s,
                None => continue,
            };

            match state.to_kind() {
                BlockKind::RedstoneWire => {
                    let changed = process_wire(&chunk_layer, pos, state);
                    if let Some((new_state, neighbors)) = changed {
                        chunk_layer.set_block(pos, new_state);
                        for n in neighbors {
                            update_queue.push(RedstoneUpdateEntry {
                                pos: n,
                                update_type: UpdateType::SignalPropagation,
                            });
                        }
                    }
                }
                BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch => {
                    let changed = process_torch(&chunk_layer, pos, state);
                    if let Some((new_state, neighbors)) = changed {
                        chunk_layer.set_block(pos, new_state);
                        for n in neighbors {
                            update_queue.push(RedstoneUpdateEntry {
                                pos: n,
                                update_type: UpdateType::SignalSource,
                            });
                        }
                    }
                }
                BlockKind::Repeater => {
                    let changed =
                        process_repeater(&chunk_layer, pos, state, system_state.current_tick);
                    if let Some((new_state, neighbors)) = changed {
                        chunk_layer.set_block(pos, new_state);
                        for n in neighbors {
                            update_queue.push(RedstoneUpdateEntry {
                                pos: n,
                                update_type: UpdateType::SignalPropagation,
                            });
                        }
                    }
                }
                BlockKind::Comparator => {
                    let changed =
                        process_comparator(&chunk_layer, pos, state, system_state.current_tick);
                    if let Some((new_state, neighbors)) = changed {
                        chunk_layer.set_block(pos, new_state);
                        for n in neighbors {
                            update_queue.push(RedstoneUpdateEntry {
                                pos: n,
                                update_type: UpdateType::SignalPropagation,
                            });
                        }
                    }
                }
                BlockKind::RedstoneLamp => {
                    if let Some(new_state) = process_lamp(&chunk_layer, pos, state) {
                        chunk_layer.set_block(pos, new_state);
                    }
                }
                BlockKind::Piston | BlockKind::StickyPiston => {
                    let changed = process_piston(&chunk_layer, pos, state);
                    if let Some((new_state, neighbors)) = changed {
                        chunk_layer.set_block(pos, new_state);
                        for n in neighbors {
                            update_queue.push(RedstoneUpdateEntry {
                                pos: n,
                                update_type: UpdateType::SignalPropagation,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn process_wire(
    chunk_layer: &ChunkLayer,
    pos: BlockPos,
    state: BlockState,
) -> Option<(BlockState, Vec<BlockPos>)> {
    let mut max_power: RedstoneStrength = 0;

    for dir in get_horizontal_directions() {
        let neighbor_pos = offset_pos(pos, dir);
        if let Some(neighbor_state) = get_block_state(chunk_layer, neighbor_pos) {
            let power = calc_signal_for_input(chunk_layer, neighbor_state, neighbor_pos, dir);
            max_power = max_power.max(power);
        }
    }

    let above_pos = offset_pos(pos, Direction::Up);
    if let Some(above_state) = get_block_state(chunk_layer, above_pos) {
        if is_redstone_conductor(above_state) {
            for dir in get_horizontal_directions() {
                let side_pos = offset_pos(above_pos, dir);
                if let Some(side_state) = get_block_state(chunk_layer, side_pos) {
                    let power = calc_signal_for_input(chunk_layer, side_state, side_pos, dir);
                    max_power = max_power.max(power);
                }
            }
        }
    }

    let below_pos = offset_pos(pos, Direction::Down);
    if let Some(below_state) = get_block_state(chunk_layer, below_pos) {
        if is_redstone_conductor(below_state) {
            let power = calc_signal_for_input(chunk_layer, below_state, below_pos, Direction::Down);
            max_power = max_power.max(power);
        }
    }

    let new_power = max_power.saturating_sub(1);
    let old_power = get_power_level(state);

    if new_power != old_power {
        let mut new_state = state;
        new_state = new_state.set(
            PropName::Power,
            PropValue::from_u16(new_power as u16).unwrap_or(PropValue::None),
        );

        let neighbors: Vec<BlockPos> = get_horizontal_directions()
            .iter()
            .map(|&dir| offset_pos(pos, dir))
            .collect();

        Some((new_state, neighbors))
    } else {
        None
    }
}

fn process_torch(
    chunk_layer: &ChunkLayer,
    pos: BlockPos,
    state: BlockState,
) -> Option<(BlockState, Vec<BlockPos>)> {
    let torch = RedstoneTorch::from_block_state(state).unwrap_or_default();

    let attach_dir = torch.get_attachment_direction();
    let attach_pos = offset_pos(pos, attach_dir);

    let mut input_power: RedstoneStrength = 0;
    if let Some(attach_state) = get_block_state(chunk_layer, attach_pos) {
        input_power = get_power_level(attach_state);
    }

    let was_lit = torch.lit;
    let should_be_lit = input_power == 0;

    if was_lit != should_be_lit {
        let mut new_state = state;
        new_state = new_state.set(
            PropName::Lit,
            if should_be_lit {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        let mut neighbors: Vec<BlockPos> = get_horizontal_directions()
            .iter()
            .map(|&dir| offset_pos(pos, dir))
            .collect();
        neighbors.push(offset_pos(pos, Direction::Up));

        Some((new_state, neighbors))
    } else {
        None
    }
}

fn process_repeater(
    chunk_layer: &ChunkLayer,
    pos: BlockPos,
    state: BlockState,
    current_tick: u64,
) -> Option<(BlockState, Vec<BlockPos>)> {
    let mut repeater = RedstoneRepeater::from_block_state(state).unwrap_or_default();

    let input_dir = repeater.get_input_direction();
    let input_pos = offset_pos(pos, input_dir);

    let mut input_power: RedstoneStrength = 0;
    if let Some(input_state) = get_block_state(chunk_layer, input_pos) {
        input_power = calc_signal_for_input(chunk_layer, input_state, input_pos, input_dir);
    }

    let lock_dirs = repeater.get_lock_directions();
    let mut lock_power: RedstoneStrength = 0;
    for &lock_dir in &lock_dirs {
        let lock_pos = offset_pos(pos, lock_dir);
        if let Some(lock_state) = get_block_state(chunk_layer, lock_pos) {
            let power = calc_signal_for_input(chunk_layer, lock_state, lock_pos, lock_dir);
            lock_power = lock_power.max(power);
        }
    }

    let inputs = vec![
        signal::RedstoneSignal {
            strength: input_power,
            signal_type: signal::SignalType::Strong,
            from_direction: Some(input_dir),
        },
        signal::RedstoneSignal {
            strength: lock_power,
            signal_type: signal::SignalType::Strong,
            from_direction: Some(lock_dirs[0]),
        },
    ];

    if repeater.update_signal(&inputs, current_tick) {
        let output_state = repeater.to_block_state();
        let output_pos = offset_pos(pos, repeater.get_output_direction());

        Some((output_state, vec![output_pos]))
    } else {
        None
    }
}

fn process_comparator(
    chunk_layer: &ChunkLayer,
    pos: BlockPos,
    state: BlockState,
    current_tick: u64,
) -> Option<(BlockState, Vec<BlockPos>)> {
    let mut comparator = RedstoneComparator::from_block_state(state).unwrap_or_default();

    let input_dir = comparator.get_input_direction();
    let input_pos = offset_pos(pos, input_dir);

    let mut input_power: RedstoneStrength = 0;
    if let Some(input_state) = get_block_state(chunk_layer, input_pos) {
        input_power = calc_signal_for_input(chunk_layer, input_state, input_pos, input_dir);
    }

    let side_dirs = comparator.get_side_directions();
    let mut side_power_1: RedstoneStrength = 0;
    let mut side_power_2: RedstoneStrength = 0;

    let side_pos_1 = offset_pos(pos, side_dirs[0]);
    if let Some(side_state) = get_block_state(chunk_layer, side_pos_1) {
        side_power_1 = calc_signal_for_input(chunk_layer, side_state, side_pos_1, side_dirs[0]);
    }

    let side_pos_2 = offset_pos(pos, side_dirs[1]);
    if let Some(side_state) = get_block_state(chunk_layer, side_pos_2) {
        side_power_2 = calc_signal_for_input(chunk_layer, side_state, side_pos_2, side_dirs[1]);
    }

    let inputs = vec![
        signal::RedstoneSignal {
            strength: input_power,
            signal_type: signal::SignalType::Strong,
            from_direction: Some(input_dir),
        },
        signal::RedstoneSignal {
            strength: side_power_1,
            signal_type: signal::SignalType::Weak,
            from_direction: Some(side_dirs[0]),
        },
        signal::RedstoneSignal {
            strength: side_power_2,
            signal_type: signal::SignalType::Weak,
            from_direction: Some(side_dirs[1]),
        },
    ];

    if comparator.update_signal(&inputs, current_tick) {
        let output_state = comparator.to_block_state();
        let output_pos = offset_pos(pos, comparator.get_output_direction());

        Some((output_state, vec![output_pos]))
    } else {
        None
    }
}

fn process_lamp(chunk_layer: &ChunkLayer, pos: BlockPos, state: BlockState) -> Option<BlockState> {
    let mut total_power: RedstoneStrength = 0;

    for dir in get_horizontal_directions() {
        let neighbor_pos = offset_pos(pos, dir);
        if let Some(neighbor_state) = get_block_state(chunk_layer, neighbor_pos) {
            let power = calc_signal_for_input(chunk_layer, neighbor_state, neighbor_pos, dir);
            total_power = total_power.max(power);
        }
    }

    let above_pos = offset_pos(pos, Direction::Up);
    if let Some(above_state) = get_block_state(chunk_layer, above_pos) {
        let power = calc_signal_for_input(chunk_layer, above_state, above_pos, Direction::Up);
        total_power = total_power.max(power);
    }

    let below_pos = offset_pos(pos, Direction::Down);
    if let Some(below_state) = get_block_state(chunk_layer, below_pos) {
        let power = calc_signal_for_input(chunk_layer, below_state, below_pos, Direction::Down);
        total_power = total_power.max(power);
    }

    let was_lit = state
        .get(PropName::Lit)
        .and_then(|v| v.to_bool())
        .unwrap_or(false);

    let should_be_lit = total_power > 0;

    if was_lit != should_be_lit {
        let mut new_state = state;
        new_state = new_state.set(
            PropName::Lit,
            if should_be_lit {
                PropValue::True
            } else {
                PropValue::False
            },
        );
        Some(new_state)
    } else {
        None
    }
}

fn process_piston(
    chunk_layer: &ChunkLayer,
    pos: BlockPos,
    state: BlockState,
) -> Option<(BlockState, Vec<BlockPos>)> {
    let mut piston = Piston::from_block_state(state).unwrap_or_default();

    let mut total_power: RedstoneStrength = 0;

    for dir in get_horizontal_directions() {
        let neighbor_pos = offset_pos(pos, dir);
        if let Some(neighbor_state) = get_block_state(chunk_layer, neighbor_pos) {
            let power = calc_strong_signal(chunk_layer, neighbor_state, neighbor_pos, dir);
            total_power = total_power.max(power);
        }
    }

    let above_pos = offset_pos(pos, Direction::Up);
    if let Some(above_state) = get_block_state(chunk_layer, above_pos) {
        let power = calc_strong_signal(chunk_layer, above_state, above_pos, Direction::Up);
        total_power = total_power.max(power);
    }

    let below_pos = offset_pos(pos, Direction::Down);
    if let Some(below_state) = get_block_state(chunk_layer, below_pos) {
        let power = calc_strong_signal(chunk_layer, below_state, below_pos, Direction::Down);
        total_power = total_power.max(power);
    }

    let was_extended = piston.is_extended();
    let should_be_extended = total_power > 0;

    if was_extended != should_be_extended {
        piston.set_extended(should_be_extended, 0);
        let output_state = piston.to_block_state();
        let output_pos = offset_pos(pos, piston.get_extension_direction());

        Some((output_state, vec![output_pos]))
    } else {
        None
    }
}

fn calc_signal_for_input(
    chunk_layer: &ChunkLayer,
    state: BlockState,
    pos: BlockPos,
    from_dir: Direction,
) -> RedstoneStrength {
    match state.to_kind() {
        BlockKind::RedstoneWire => get_power_level(state),
        BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch => {
            let torch = RedstoneTorch::from_block_state(state).unwrap_or_default();
            if torch.lit {
                MAX_SIGNAL
            } else {
                0
            }
        }
        BlockKind::Repeater => {
            let repeater = RedstoneRepeater::from_block_state(state).unwrap_or_default();
            if repeater.powered && from_dir == repeater.get_output_direction() {
                MAX_SIGNAL
            } else {
                0
            }
        }
        BlockKind::Comparator => {
            let comparator = RedstoneComparator::from_block_state(state).unwrap_or_default();
            if comparator.powered && from_dir == comparator.get_output_direction() {
                comparator.output_power
            } else {
                0
            }
        }
        BlockKind::RedstoneBlock => MAX_SIGNAL,
        _ => {
            if is_redstone_conductor(state) {
                let above_pos = offset_pos(pos, Direction::Up);
                if let Some(above_state) = get_block_state(chunk_layer, above_pos) {
                    if is_redstone_conductor(above_state) {
                        let mut max_side: RedstoneStrength = 0;
                        for dir in get_horizontal_directions() {
                            let side_pos = offset_pos(above_pos, dir);
                            if let Some(side_state) = get_block_state(chunk_layer, side_pos) {
                                let power =
                                    calc_signal_for_input(chunk_layer, side_state, side_pos, dir);
                                max_side = max_side.max(power);
                            }
                        }
                        return max_side;
                    }
                }
                0
            } else {
                0
            }
        }
    }
}

fn calc_strong_signal(
    _chunk_layer: &ChunkLayer,
    state: BlockState,
    _pos: BlockPos,
    _from_dir: Direction,
) -> RedstoneStrength {
    match state.to_kind() {
        BlockKind::RedstoneWire => {
            let power = get_power_level(state);
            if power > 0 {
                power
            } else {
                0
            }
        }
        BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch => {
            let torch = RedstoneTorch::from_block_state(state).unwrap_or_default();
            if torch.lit {
                MAX_SIGNAL
            } else {
                0
            }
        }
        BlockKind::Repeater => {
            let repeater = RedstoneRepeater::from_block_state(state).unwrap_or_default();
            if repeater.powered {
                MAX_SIGNAL
            } else {
                0
            }
        }
        BlockKind::Comparator => {
            let comparator = RedstoneComparator::from_block_state(state).unwrap_or_default();
            if comparator.powered {
                comparator.output_power
            } else {
                0
            }
        }
        BlockKind::RedstoneBlock => MAX_SIGNAL,
        _ => 0,
    }
}

pub fn trigger_redstone_update(pos: BlockPos, update_queue: &mut RedstoneUpdateQueue) {
    update_queue.push(RedstoneUpdateEntry {
        pos,
        update_type: UpdateType::NeighborUpdate,
    });

    for dir in get_horizontal_directions() {
        let neighbor_pos = offset_pos(pos, dir);
        update_queue.push(RedstoneUpdateEntry {
            pos: neighbor_pos,
            update_type: UpdateType::NeighborUpdate,
        });
    }

    let above_pos = offset_pos(pos, Direction::Up);
    update_queue.push(RedstoneUpdateEntry {
        pos: above_pos,
        update_type: UpdateType::NeighborUpdate,
    });

    let below_pos = offset_pos(pos, Direction::Down);
    update_queue.push(RedstoneUpdateEntry {
        pos: below_pos,
        update_type: UpdateType::NeighborUpdate,
    });
}

pub fn is_block_powered_at(chunk_layer: &ChunkLayer, pos: BlockPos) -> bool {
    for dir in get_horizontal_directions() {
        let neighbor_pos = offset_pos(pos, dir);
        if let Some(neighbor_state) = get_block_state(chunk_layer, neighbor_pos) {
            let power = calc_signal_for_input(chunk_layer, neighbor_state, neighbor_pos, dir);
            if power > 0 {
                return true;
            }
        }
    }

    let above_pos = offset_pos(pos, Direction::Up);
    if let Some(above_state) = get_block_state(chunk_layer, above_pos) {
        let power = calc_signal_for_input(chunk_layer, above_state, above_pos, Direction::Up);
        if power > 0 {
            return true;
        }
    }

    false
}

pub fn get_redstone_signal_at(chunk_layer: &ChunkLayer, pos: BlockPos) -> RedstoneStrength {
    if let Some(state) = get_block_state(chunk_layer, pos) {
        match state.to_kind() {
            BlockKind::RedstoneWire => get_power_level(state),
            BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch => {
                let torch = RedstoneTorch::from_block_state(state).unwrap_or_default();
                if torch.lit {
                    MAX_SIGNAL
                } else {
                    0
                }
            }
            BlockKind::Repeater => {
                let repeater = RedstoneRepeater::from_block_state(state).unwrap_or_default();
                if repeater.powered {
                    MAX_SIGNAL
                } else {
                    0
                }
            }
            BlockKind::Comparator => {
                let comparator = RedstoneComparator::from_block_state(state).unwrap_or_default();
                if comparator.powered {
                    comparator.output_power
                } else {
                    0
                }
            }
            BlockKind::RedstoneBlock => MAX_SIGNAL,
            _ => 0,
        }
    } else {
        0
    }
}
