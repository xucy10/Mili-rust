use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::{
    get_opposite_direction, offset_pos, RedstoneSignal, RedstoneStrength, MAX_SIGNAL,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ComparatorMode {
    Compare,
    Subtract,
}

#[derive(Component, Debug)]
pub struct RedstoneComparator {
    pub mode: ComparatorMode,
    pub powered: bool,
    pub facing: Direction,
    pub input_power: RedstoneStrength,
    pub side_power_1: RedstoneStrength,
    pub side_power_2: RedstoneStrength,
    pub output_power: RedstoneStrength,
    pub last_tick: u64,
}

impl Default for RedstoneComparator {
    fn default() -> Self {
        Self {
            mode: ComparatorMode::Compare,
            powered: false,
            facing: Direction::North,
            input_power: 0,
            side_power_1: 0,
            side_power_2: 0,
            output_power: 0,
            last_tick: 0,
        }
    }
}

impl RedstoneComparator {
    pub fn new(facing: Direction) -> Self {
        Self {
            facing,
            ..Default::default()
        }
    }

    pub fn compare_mode() -> Self {
        Self {
            mode: ComparatorMode::Compare,
            ..Default::default()
        }
    }

    pub fn subtract_mode() -> Self {
        Self {
            mode: ComparatorMode::Subtract,
            ..Default::default()
        }
    }

    pub fn get_input_direction(&self) -> Direction {
        get_opposite_direction(self.facing)
    }

    pub fn get_output_direction(&self) -> Direction {
        self.facing
    }

    pub fn get_side_directions(&self) -> [Direction; 2] {
        match self.facing {
            Direction::North | Direction::South => [Direction::West, Direction::East],
            Direction::West | Direction::East => [Direction::North, Direction::South],
            _ => [Direction::North, Direction::South],
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            ComparatorMode::Compare => ComparatorMode::Subtract,
            ComparatorMode::Subtract => ComparatorMode::Compare,
        };
    }

    pub fn set_mode(&mut self, mode: ComparatorMode) {
        self.mode = mode;
    }

    pub fn get_output_signal(&self) -> RedstoneStrength {
        if self.powered {
            self.output_power
        } else {
            0
        }
    }

    pub fn get_output_for_direction(&self, dir: Direction) -> RedstoneStrength {
        if dir == self.facing && self.powered {
            self.output_power
        } else {
            0
        }
    }

    pub fn calculate_output(&self) -> RedstoneStrength {
        let side_power = self.side_power_1.max(self.side_power_2);

        match self.mode {
            ComparatorMode::Compare => {
                if self.input_power >= side_power {
                    self.input_power
                } else {
                    0
                }
            }
            ComparatorMode::Subtract => {
                let output = self.input_power as i16 - side_power as i16;
                output.max(0) as RedstoneStrength
            }
        }
    }

    pub fn update_signal(&mut self, inputs: &[RedstoneSignal], current_tick: u64) -> bool {
        let input_dir = self.get_input_direction();
        let side_dirs = self.get_side_directions();

        let input_signal = inputs
            .iter()
            .find(|s| s.from_direction == Some(input_dir))
            .map(|s| s.strength)
            .unwrap_or(0);

        let side_signal_1 = inputs
            .iter()
            .find(|s| s.from_direction == Some(side_dirs[0]))
            .map(|s| s.strength)
            .unwrap_or(0);

        let side_signal_2 = inputs
            .iter()
            .find(|s| s.from_direction == Some(side_dirs[1]))
            .map(|s| s.strength)
            .unwrap_or(0);

        self.input_power = input_signal;
        self.side_power_1 = side_signal_1;
        self.side_power_2 = side_signal_2;

        let new_output = self.calculate_output();
        let new_powered = new_output > 0;

        if self.powered != new_powered || self.output_power != new_output {
            self.powered = new_powered;
            self.output_power = new_output;
            self.last_tick = current_tick;
            return true;
        }

        false
    }

    pub fn needs_scheduled_tick(&self) -> bool {
        false
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = BlockState::COMPARATOR;

        state = state.set(
            PropName::Facing,
            match self.facing {
                Direction::North => PropValue::North,
                Direction::South => PropValue::South,
                Direction::West => PropValue::West,
                Direction::East => PropValue::East,
                _ => PropValue::North,
            },
        );

        state = state.set(
            PropName::Mode,
            match self.mode {
                ComparatorMode::Compare => PropValue::Compare,
                ComparatorMode::Subtract => PropValue::Subtract,
            },
        );

        state = state.set(
            PropName::Powered,
            if self.powered {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        state
    }

    pub fn from_block_state(state: BlockState) -> Option<Self> {
        if state.to_kind() != BlockKind::Comparator {
            return None;
        }

        let facing = state.get(PropName::Facing).and_then(|v| match v {
            PropValue::North => Some(Direction::North),
            PropValue::South => Some(Direction::South),
            PropValue::West => Some(Direction::West),
            PropValue::East => Some(Direction::East),
            _ => None,
        })?;

        let mode = state.get(PropName::Mode).and_then(|v| match v {
            PropValue::Compare => Some(ComparatorMode::Compare),
            PropValue::Subtract => Some(ComparatorMode::Subtract),
            _ => None,
        })?;

        let powered = state
            .get(PropName::Powered)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            mode,
            powered,
            facing,
            input_power: 0,
            side_power_1: 0,
            side_power_2: 0,
            output_power: 0,
            last_tick: 0,
        })
    }
}

pub fn is_comparator(state: BlockState) -> bool {
    state.to_kind() == BlockKind::Comparator
}

pub fn get_comparator_signal(pos: BlockPos, state: BlockState) -> RedstoneSignal {
    if !is_comparator(state) {
        return RedstoneSignal::none();
    }

    let comparator = RedstoneComparator::from_block_state(state).unwrap_or_default();

    if comparator.powered {
        RedstoneSignal {
            strength: comparator.output_power,
            signal_type: SignalType::Strong,
            from_direction: None,
        }
    } else {
        RedstoneSignal::none()
    }
}
