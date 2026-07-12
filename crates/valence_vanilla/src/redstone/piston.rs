use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::{get_opposite_direction, offset_pos, RedstoneStrength};

#[derive(Component, Debug)]
pub struct Piston {
    pub sticky: bool,
    pub facing: Direction,
    pub extended: bool,
    pub power: RedstoneStrength,
    pub extending_tick: u64,
    pub retracting_tick: u64,
    pub push_limit: u8,
}

impl Default for Piston {
    fn default() -> Self {
        Self {
            sticky: false,
            facing: Direction::North,
            extended: false,
            power: 0,
            extending_tick: 0,
            retracting_tick: 0,
            push_limit: 12,
        }
    }
}

impl Piston {
    pub fn new(facing: Direction) -> Self {
        Self {
            facing,
            ..Default::default()
        }
    }

    pub fn sticky(facing: Direction) -> Self {
        Self {
            sticky: true,
            facing,
            ..Default::default()
        }
    }

    pub fn is_sticky(&self) -> bool {
        self.sticky
    }

    pub fn is_extended(&self) -> bool {
        self.extended
    }

    pub fn set_extended(&mut self, extended: bool, current_tick: u64) {
        if extended && !self.extended {
            self.extending_tick = current_tick;
        } else if !extended && self.extended {
            self.retracting_tick = current_tick;
        }
        self.extended = extended;
    }

    pub fn get_extension_direction(&self) -> Direction {
        self.facing
    }

    pub fn get_retraction_direction(&self) -> Direction {
        get_opposite_direction(self.facing)
    }

    pub fn get_head_position(&self, base_pos: BlockPos) -> BlockPos {
        if self.extended {
            offset_pos(base_pos, self.facing)
        } else {
            base_pos
        }
    }

    pub fn get_blocks_to_push(&self, base_pos: BlockPos) -> Vec<BlockPos> {
        let mut blocks = Vec::new();

        if !self.extended {
            return blocks;
        }

        let head_pos = self.get_head_position(base_pos);
        let mut current_pos = head_pos;

        for _ in 0..self.push_limit {
            let next_pos = offset_pos(current_pos, self.facing);
            blocks.push(current_pos);
            current_pos = next_pos;
        }

        blocks
    }

    pub fn can_push_block(&self, state: BlockState) -> bool {
        if state.is_air() {
            return true;
        }

        if state.blocks_motion() {
            return false;
        }

        match state.to_kind() {
            BlockKind::Piston | BlockKind::StickyPiston => {
                let piston = Piston::from_block_state(state).unwrap_or_default();
                !piston.extended
            }
            _ => true,
        }
    }

    pub fn update_signal(&mut self, power: RedstoneStrength, current_tick: u64) -> bool {
        let was_extended = self.extended;
        self.power = power;

        if power > 0 && !self.extended {
            self.set_extended(true, current_tick);
        } else if power == 0 && self.extended {
            self.set_extended(false, current_tick);
        }

        was_extended != self.extended
    }

    pub fn needs_scheduled_tick(&self) -> bool {
        let target_extended = self.power > 0;
        self.extended != target_extended
    }

    pub fn to_block_state(&self) -> BlockState {
        let state = if self.sticky {
            BlockState::STICKY_PISTON
        } else {
            BlockState::PISTON
        };

        let mut state = state;

        state = state.set(
            PropName::Facing,
            match self.facing {
                Direction::North => PropValue::North,
                Direction::South => PropValue::South,
                Direction::West => PropValue::West,
                Direction::East => PropValue::East,
                Direction::Up => PropValue::Up,
                Direction::Down => PropValue::Down,
            },
        );

        state = state.set(
            PropName::Extended,
            if self.extended {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        state
    }

    pub fn from_block_state(state: BlockState) -> Option<Self> {
        let sticky = match state.to_kind() {
            BlockKind::StickyPiston => true,
            BlockKind::Piston => false,
            _ => return None,
        };

        let facing = state.get(PropName::Facing).and_then(|v| match v {
            PropValue::North => Some(Direction::North),
            PropValue::South => Some(Direction::South),
            PropValue::West => Some(Direction::West),
            PropValue::East => Some(Direction::East),
            PropValue::Up => Some(Direction::Up),
            PropValue::Down => Some(Direction::Down),
            _ => None,
        })?;

        let extended = state
            .get(PropName::Extended)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            sticky,
            facing,
            extended,
            power: 0,
            extending_tick: 0,
            retracting_tick: 0,
            push_limit: 12,
        })
    }
}

#[derive(Component, Debug)]
pub struct PistonHead {
    pub facing: Direction,
    pub piston_type: PistonType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PistonType {
    Normal,
    Sticky,
}

impl PistonHead {
    pub fn new(facing: Direction, piston_type: PistonType) -> Self {
        Self {
            facing,
            piston_type,
        }
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = match self.piston_type {
            PistonType::Normal => BlockState::PISTON_HEAD,
            PistonType::Sticky => BlockState::PISTON_HEAD,
        };

        state = state.set(
            PropName::Facing,
            match self.facing {
                Direction::North => PropValue::North,
                Direction::South => PropValue::South,
                Direction::West => PropValue::West,
                Direction::East => PropValue::East,
                Direction::Up => PropValue::Up,
                Direction::Down => PropValue::Down,
            },
        );

        state = state.set(
            PropName::Type,
            match self.piston_type {
                PistonType::Normal => PropValue::Normal,
                PistonType::Sticky => PropValue::Sticky,
            },
        );

        state
    }

    pub fn from_block_state(state: BlockState) -> Option<Self> {
        if state.to_kind() != BlockKind::PistonHead {
            return None;
        }

        let facing = state.get(PropName::Facing).and_then(|v| match v {
            PropValue::North => Some(Direction::North),
            PropValue::South => Some(Direction::South),
            PropValue::West => Some(Direction::West),
            PropValue::East => Some(Direction::East),
            PropValue::Up => Some(Direction::Up),
            PropValue::Down => Some(Direction::Down),
            _ => None,
        })?;

        let piston_type = state.get(PropName::Type).and_then(|v| match v {
            PropValue::Normal => Some(PistonType::Normal),
            PropValue::Sticky => Some(PistonType::Sticky),
            _ => None,
        })?;

        Some(Self {
            facing,
            piston_type,
        })
    }
}

pub fn is_piston(state: BlockState) -> bool {
    matches!(state.to_kind(), BlockKind::Piston | BlockKind::StickyPiston)
}

pub fn is_piston_head(state: BlockState) -> bool {
    state.to_kind() == BlockKind::PistonHead
}

pub fn get_piston_power(state: BlockState) -> RedstoneStrength {
    if !is_piston(state) {
        return 0;
    }

    if let Some(extended) = state.get(PropName::Extended) {
        if extended.to_bool().unwrap_or(false) {
            15
        } else {
            0
        }
    } else {
        0
    }
}
