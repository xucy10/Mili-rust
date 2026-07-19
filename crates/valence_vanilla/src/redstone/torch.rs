use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::Direction;

use super::signal::MAX_SIGNAL;

pub struct RedstoneTorch {
    pub lit: bool,
}

impl Default for RedstoneTorch {
    fn default() -> Self {
        Self { lit: true }
    }
}

impl RedstoneTorch {
    pub fn from_block_state(state: BlockState) -> Option<Self> {
        let kind = state.to_kind();
        if kind != BlockKind::RedstoneTorch && kind != BlockKind::RedstoneWallTorch {
            return None;
        }

        let lit = state
            .get(PropName::Lit)
            .and_then(|v| v.to_bool())
            .unwrap_or(true);

        Some(Self { lit })
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = if self.lit {
            BlockState::REDSTONE_TORCH
        } else {
            BlockState::REDSTONE_WALL_TORCH
        };

        state = state.set(
            PropName::Lit,
            if self.lit {
                PropValue::True
            } else {
                PropValue::False
            },
        );

        state
    }

    pub fn get_attachment_direction(&self) -> Direction {
        Direction::Down
    }

    pub fn get_output_power(&self) -> u8 {
        if self.lit {
            MAX_SIGNAL
        } else {
            0
        }
    }
}