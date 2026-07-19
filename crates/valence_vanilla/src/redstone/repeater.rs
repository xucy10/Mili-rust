use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::Direction;

pub struct RedstoneRepeater {
    pub facing: Direction,
    pub delay: u8,
    pub powered: bool,
    pub locked: bool,
}

impl Default for RedstoneRepeater {
    fn default() -> Self {
        Self {
            facing: Direction::South,
            delay: 1,
            powered: false,
            locked: false,
        }
    }
}

impl RedstoneRepeater {
    pub fn from_block_state(state: BlockState) -> Option<Self> {
        if state.to_kind() != BlockKind::Repeater {
            return None;
        }

        let facing = match state.get(PropName::Facing) {
            Some(PropValue::North) => Direction::North,
            Some(PropValue::South) => Direction::South,
            Some(PropValue::West) => Direction::West,
            Some(PropValue::East) => Direction::East,
            _ => Direction::South,
        };

        let delay = state
            .get(PropName::Delay)
            .and_then(|v| v.to_u16())
            .unwrap_or(1) as u8;

        let powered = state
            .get(PropName::Powered)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        let locked = state
            .get(PropName::Locked)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            facing,
            delay,
            powered,
            locked,
        })
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = BlockState::REPEATER;

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
            PropName::Delay,
            PropValue::from_u16(self.delay as u16).unwrap_or(PropValue::_1),
        );

        state = state.set(
            PropName::Powered,
            if self.powered {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        state = state.set(
            PropName::Locked,
            if self.locked {
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

    pub fn get_lock_directions(&self) -> [Direction; 2] {
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
        if self.locked {
            return false;
        }

        let main_input = inputs.first().map(|s| s.strength).unwrap_or(0);
        let lock_power = inputs.get(1).map(|s| s.strength).unwrap_or(0);

        let should_be_powered = main_input > 0;
        let should_be_locked = lock_power > 0;

        let changed = self.powered != should_be_powered || self.locked != should_be_locked;

        self.powered = should_be_powered;
        self.locked = should_be_locked;

        changed
    }
}