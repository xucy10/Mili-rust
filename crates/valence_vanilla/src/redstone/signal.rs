use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName};
use valence_protocol::{BlockPos, Direction};

pub type RedstoneStrength = u8;
pub const MAX_SIGNAL: RedstoneStrength = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalType {
    Strong,
    Weak,
}

#[derive(Clone, Copy, Debug)]
pub struct RedstoneSignal {
    pub strength: RedstoneStrength,
    pub signal_type: SignalType,
    pub from_direction: Option<Direction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateType {
    NeighborUpdate,
    SignalPropagation,
    SignalSource,
}

#[derive(Clone, Copy, Debug)]
pub struct RedstoneUpdateEntry {
    pub pos: BlockPos,
    pub update_type: UpdateType,
}

#[derive(Resource, Default, Debug)]
pub struct RedstoneUpdateQueue {
    entries: Vec<RedstoneUpdateEntry>,
}

impl RedstoneUpdateQueue {
    pub fn push(&mut self, entry: RedstoneUpdateEntry) {
        self.entries.push(entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = &RedstoneUpdateEntry> {
        self.entries.iter()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
    pos.get_in_direction(dir)
}

pub fn get_power_level(state: BlockState) -> RedstoneStrength {
    if state.to_kind() == BlockKind::RedstoneWire {
        state
            .get(PropName::Power)
            .and_then(|v| v.to_u16())
            .unwrap_or(0) as RedstoneStrength
    } else {
        0
    }
}

pub fn is_redstone_conductor(state: BlockState) -> bool {
    let kind = state.to_kind();
    !matches!(
        kind,
        BlockKind::Air
            | BlockKind::CaveAir
            | BlockKind::VoidAir
            | BlockKind::RedstoneWire
            | BlockKind::TallGrass
            | BlockKind::Grass
            | BlockKind::Fern
            | BlockKind::DeadBush
            | BlockKind::Dandelion
            | BlockKind::Poppy
            | BlockKind::Glass
            | BlockKind::GlassPane
    )
}