use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::Direction;

pub struct Piston {
    pub facing: Direction,
    pub extended: bool,
    pub is_sticky: bool,
}

impl Default for Piston {
    fn default() -> Self {
        Self {
            facing: Direction::North,
            extended: false,
            is_sticky: false,
        }
    }
}

impl Piston {
    pub fn from_block_state(state: BlockState) -> Option<Self> {
        let kind = state.to_kind();
        if kind != BlockKind::Piston && kind != BlockKind::StickyPiston {
            return None;
        }

        let facing = match state.get(PropName::Facing) {
            Some(PropValue::North) => Direction::North,
            Some(PropValue::South) => Direction::South,
            Some(PropValue::West) => Direction::West,
            Some(PropValue::East) => Direction::East,
            Some(PropValue::Up) => Direction::Up,
            Some(PropValue::Down) => Direction::Down,
            _ => Direction::North,
        };

        let extended = state
            .get(PropName::Extended)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        let is_sticky = kind == BlockKind::StickyPiston;

        Some(Self {
            facing,
            extended,
            is_sticky,
        })
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = if self.is_sticky {
            BlockState::STICKY_PISTON
        } else {
            BlockState::PISTON
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
            PropName::Extended,
            if self.extended {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        state
    }

    pub fn is_extended(&self) -> bool {
        self.extended
    }

    pub fn set_extended(&mut self, extended: bool, _push_limit: u32) {
        self.extended = extended;
    }

    pub fn get_extension_direction(&self) -> Direction {
        self.facing
    }
}