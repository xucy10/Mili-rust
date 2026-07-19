use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::Direction;

use super::signal::{RedstoneStrength, MAX_SIGNAL};

pub struct RedstoneComparator {
    pub facing: Direction,
    pub mode: ComparatorMode,
    pub powered: bool,
    pub output_power: RedstoneStrength,
    pub last_input_power: RedstoneStrength,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComparatorMode {
    Compare,
    Subtract,
}

impl Default for RedstoneComparator {
    fn default() -> Self {
        Self {
            facing: Direction::South,
            mode: ComparatorMode::Compare,
            powered: false,
            output_power: 0,
            last_input_power: 0,
        }
    }
}

impl RedstoneComparator {
    pub fn from_block_state(state: BlockState) -> Option<Self> {
        if state.to_kind() != BlockKind::Comparator {
            return None;
        }

        let facing = match state.get(PropName::Facing) {
            Some(PropValue::North) => Direction::North,
            Some(PropValue::South) => Direction::South,
            Some(PropValue::West) => Direction::West,
            Some(PropValue::East) => Direction::East,
            _ => Direction::South,
        };

        let mode = match state.get(PropName::Mode) {
            Some(PropValue::Compare) => ComparatorMode::Compare,
            Some(PropValue::Subtract) => ComparatorMode::Subtract,
            _ => ComparatorMode::Compare,
        };

        let powered = state
            .get(PropName::Powered)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            facing,
            mode,
            powered,
            output_power: if powered { MAX_SIGNAL } else { 0 },
            last_input_power: 0,
        })
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
                _ => PropValue::South,
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

    pub fn get_input_direction(&self) -> Direction {
        match self.facing {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
            _ => Direction::North,
        }
    }

    pub fn get_output_direction(&self) -> Direction {
        self.facing
    }

    pub fn get_side_directions(&self) -> [Direction; 2] {
        match self.facing {
            Direction::North | Direction::South => [Direction::West, Direction::East],
            Direction::West | Direction::East => [Direction::North, Direction::South],
            _ => [Direction::West, Direction::East],
        }
    }

    pub fn update_signal(
        &mut self,
        inputs: &[super::signal::RedstoneSignal],
        _current_tick: u64,
    ) -> bool {
        let main_input = inputs.first().map(|s| s.strength).unwrap_or(0);
        let side_1 = inputs.get(1).map(|s| s.strength).unwrap_or(0);
        let side_2 = inputs.get(2).map(|s| s.strength).unwrap_or(0);

        let output = match self.mode {
            ComparatorMode::Compare => {
                if main_input >= side_1.max(side_2) {
                    main_input
                } else {
                    0
                }
            }
            ComparatorMode::Subtract => main_input.saturating_sub(side_1.max(side_2)),
        };

        let new_powered = output > 0;
        let changed = self.powered != new_powered || self.output_power != output;

        self.powered = new_powered;
        self.output_power = output;
        self.last_input_power = main_input;

        changed
    }
}