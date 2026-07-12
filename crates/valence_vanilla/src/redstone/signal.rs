use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

pub type RedstoneStrength = u8;

pub const MAX_SIGNAL: RedstoneStrength = 15;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalType {
    None,
    Weak,
    Strong,
}

#[derive(Clone, Copy, Debug)]
pub struct RedstoneSignal {
    pub strength: RedstoneStrength,
    pub signal_type: SignalType,
    pub from_direction: Option<Direction>,
}

impl RedstoneSignal {
    pub fn none() -> Self {
        Self {
            strength: 0,
            signal_type: SignalType::None,
            from_direction: None,
        }
    }

    pub fn strong(strength: RedstoneStrength, from: Direction) -> Self {
        Self {
            strength,
            signal_type: SignalType::Strong,
            from_direction: Some(from),
        }
    }

    pub fn weak(strength: RedstoneStrength, from: Direction) -> Self {
        Self {
            strength,
            signal_type: SignalType::Weak,
            from_direction: Some(from),
        }
    }

    pub fn is_active(&self) -> bool {
        self.strength > 0
    }
}

pub fn get_direction_offset(dir: Direction) -> (i32, i32, i32) {
    match dir {
        Direction::Down => (0, -1, 0),
        Direction::Up => (0, 1, 0),
        Direction::North => (0, 0, -1),
        Direction::South => (0, 0, 1),
        Direction::West => (-1, 0, 0),
        Direction::East => (1, 0, 0),
    }
}

pub fn get_opposite_direction(dir: Direction) -> Direction {
    match dir {
        Direction::Down => Direction::Up,
        Direction::Up => Direction::Down,
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::West => Direction::East,
        Direction::East => Direction::West,
    }
}

pub fn get_horizontal_directions() -> [Direction; 4] {
    [
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]
}

pub fn offset_pos(pos: BlockPos, dir: Direction) -> BlockPos {
    let (dx, dy, dz) = get_direction_offset(dir);
    BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz)
}

pub fn is_redstone_conductor(state: BlockState) -> bool {
    state.to_kind() == BlockKind::RedstoneBlock || (state.is_opaque() && state.blocks_motion())
}

pub fn is_wire(state: BlockState) -> bool {
    state.to_kind() == BlockKind::RedstoneWire
}

pub fn is_torch(state: BlockState) -> bool {
    matches!(
        state.to_kind(),
        BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch
    )
}

pub fn is_repeater(state: BlockState) -> bool {
    state.to_kind() == BlockKind::Repeater
}

pub fn is_comparator(state: BlockState) -> bool {
    state.to_kind() == BlockKind::Comparator
}

pub fn is_lamp(state: BlockState) -> bool {
    state.to_kind() == BlockKind::RedstoneLamp
}

pub fn is_piston(state: BlockState) -> bool {
    matches!(state.to_kind(), BlockKind::Piston | BlockKind::StickyPiston)
}

pub fn is_redstone_component(state: BlockState) -> bool {
    is_wire(state)
        || is_torch(state)
        || is_repeater(state)
        || is_comparator(state)
        || is_lamp(state)
        || is_piston(state)
}

pub fn get_power_level(state: BlockState) -> RedstoneStrength {
    if let Some(power_val) = state.get(PropName::Power) {
        power_val.to_u16().unwrap_or(0) as RedstoneStrength
    } else {
        0
    }
}

pub fn is_powered(state: BlockState) -> bool {
    if let Some(powered) = state.get(PropName::Powered) {
        powered.to_bool().unwrap_or(false)
    } else if let Some(lit) = state.get(PropName::Lit) {
        lit.to_bool().unwrap_or(false)
    } else {
        false
    }
}

pub fn get_strong_signal(state: BlockState, from: Direction) -> RedstoneStrength {
    match state.to_kind() {
        BlockKind::RedstoneWire => {
            let power = get_power_level(state);
            if power > 0 {
                power
            } else {
                0
            }
        }
        BlockKind::RedstoneTorch => {
            if is_powered(state) {
                0
            } else {
                15
            }
        }
        BlockKind::RedstoneWallTorch => {
            if is_powered(state) {
                0
            } else {
                15
            }
        }
        BlockKind::Repeater => {
            if is_powered(state) {
                15
            } else {
                0
            }
        }
        BlockKind::Comparator => {
            if is_powered(state) {
                get_comparator_output(state)
            } else {
                0
            }
        }
        BlockKind::RedstoneBlock => 15,
        _ => 0,
    }
}

pub fn get_weak_signal(state: BlockState, from: Direction) -> RedstoneStrength {
    match state.to_kind() {
        BlockKind::RedstoneWire => get_power_level(state),
        BlockKind::RedstoneTorch => {
            if is_powered(state) {
                0
            } else {
                15
            }
        }
        BlockKind::RedstoneWallTorch => {
            if is_powered(state) {
                0
            } else {
                15
            }
        }
        BlockKind::Repeater => {
            if is_powered(state) {
                15
            } else {
                0
            }
        }
        BlockKind::Comparator => {
            if is_powered(state) {
                get_comparator_output(state)
            } else {
                0
            }
        }
        BlockKind::RedstoneBlock => 15,
        BlockKind::RedstoneLamp => {
            if is_powered(state) {
                15
            } else {
                0
            }
        }
        _ => 0,
    }
}

fn get_comparator_output(state: BlockState) -> RedstoneStrength {
    if let Some(mode) = state.get(PropName::Mode) {
        match mode {
            PropValue::Compare => 0,
            PropValue::Subtract => 0,
            _ => 0,
        }
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateType {
    SignalSource,
    SignalPropagation,
    NeighborUpdate,
}

#[derive(Clone, Copy, Debug)]
pub struct RedstoneUpdateEntry {
    pub pos: BlockPos,
    pub update_type: UpdateType,
}

#[derive(Resource)]
pub struct RedstoneUpdateQueue {
    entries: Vec<RedstoneUpdateEntry>,
    processed: Vec<BlockPos>,
}

impl RedstoneUpdateQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            processed: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: RedstoneUpdateEntry) {
        if !self.processed.contains(&entry.pos) {
            self.entries.push(entry);
            self.processed.push(entry.pos);
        }
    }

    pub fn pop(&mut self) -> Option<RedstoneUpdateEntry> {
        self.entries.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.processed.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = &RedstoneUpdateEntry> {
        self.entries.iter()
    }
}

impl Default for RedstoneUpdateQueue {
    fn default() -> Self {
        Self::new()
    }
}
