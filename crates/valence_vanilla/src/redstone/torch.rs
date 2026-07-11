use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::{
    get_direction_offset, get_opposite_direction, offset_pos, RedstoneSignal, RedstoneStrength,
    SignalType, MAX_SIGNAL,
};

#[derive(Component, Debug)]
pub struct RedstoneTorch {
    pub lit: bool,
    pub power: RedstoneStrength,
    pub facing: Option<Direction>,
}

impl Default for RedstoneTorch {
    fn default() -> Self {
        Self {
            lit: true,
            power: MAX_SIGNAL,
            facing: None,
        }
    }
}

impl RedstoneTorch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wall(facing: Direction) -> Self {
        Self {
            lit: true,
            power: MAX_SIGNAL,
            facing: Some(facing),
        }
    }

    pub fn is_lit(&self) -> bool {
        self.lit
    }

    pub fn set_lit(&mut self, lit: bool) {
        self.lit = lit;
        if lit {
            self.power = MAX_SIGNAL;
        } else {
            self.power = 0;
        }
    }

    pub fn get_output_signal(&self) -> RedstoneStrength {
        if self.lit {
            self.power
        } else {
            0
        }
    }

    pub fn get_output_for_direction(&self, dir: Direction) -> RedstoneStrength {
        if !self.lit {
            return 0;
        }

        if let Some(facing) = self.facing {
            if dir == facing || dir == get_opposite_direction(facing) {
                return 0;
            }
        } else {
            if dir == Direction::Up || dir == Direction::Down {
                return 0;
            }
        }

        self.power
    }

    pub fn update_signal(&mut self, inputs: &[RedstoneSignal]) -> bool {
        let powered = inputs
            .iter()
            .any(|s| s.strength > 0 && s.from_direction != Some(Direction::Up));

        let new_lit = !powered;

        if self.lit != new_lit {
            self.set_lit(new_lit);
            true
        } else {
            false
        }
    }

    pub fn to_block_state(&self) -> BlockState {
        if let Some(facing) = self.facing {
            let mut state = BlockState::REDSTONE_WALL_TORCH;

            state = state.set(
                PropName::Facing,
                match facing {
                    Direction::North => PropValue::North,
                    Direction::South => PropValue::South,
                    Direction::West => PropValue::West,
                    Direction::East => PropValue::East,
                    _ => PropValue::North,
                },
            );

            state = state.set(
                PropName::Lit,
                if self.lit {
                    PropValue::True
                } else {
                    PropValue::False
                },
            );

            state
        } else {
            let mut state = BlockState::REDSTONE_TORCH;

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
    }

    pub fn from_block_state(state: BlockState) -> Option<Self> {
        match state.to_kind() {
            BlockKind::RedstoneTorch => {
                let lit = state
                    .get(PropName::Lit)
                    .and_then(|v| v.to_bool())
                    .unwrap_or(true);

                Some(Self {
                    lit,
                    power: if lit { MAX_SIGNAL } else { 0 },
                    facing: None,
                })
            }
            BlockKind::RedstoneWallTorch => {
                let lit = state
                    .get(PropName::Lit)
                    .and_then(|v| v.to_bool())
                    .unwrap_or(true);

                let facing = state.get(PropName::Facing).and_then(|v| match v {
                    PropValue::North => Some(Direction::North),
                    PropValue::South => Some(Direction::South),
                    PropValue::West => Some(Direction::West),
                    PropValue::East => Some(Direction::East),
                    _ => None,
                });

                Some(Self {
                    lit,
                    power: if lit { MAX_SIGNAL } else { 0 },
                    facing,
                })
            }
            _ => None,
        }
    }

    pub fn get_attachment_direction(&self) -> Direction {
        if let Some(facing) = self.facing {
            get_opposite_direction(facing)
        } else {
            Direction::Down
        }
    }

    pub fn is_valid_attachment(&self, attached_state: BlockState) -> bool {
        let attach_dir = self.get_attachment_direction();
        let attach_pos = offset_pos(BlockPos::new(0, 0, 0), attach_dir);

        if attach_dir == Direction::Down {
            return attached_state.is_opaque();
        }

        if attach_dir == Direction::Up {
            return false;
        }

        attached_state.is_opaque() || is_wall_torch_support(attached_state, attach_dir)
    }
}

fn is_wall_torch_support(state: BlockState, facing: Direction) -> bool {
    match state.to_kind() {
        BlockKind::Fence | BlockKind::CobblestoneWall => true,
        _ => false,
    }
}

pub fn is_torch(state: BlockState) -> bool {
    matches!(
        state.to_kind(),
        BlockKind::RedstoneTorch | BlockKind::RedstoneWallTorch
    )
}

pub fn get_torch_signal(pos: BlockPos, state: BlockState) -> RedstoneSignal {
    if !is_torch(state) {
        return RedstoneSignal::none();
    }

    let torch = RedstoneTorch::from_block_state(state).unwrap_or_default();

    if torch.lit {
        RedstoneSignal {
            strength: MAX_SIGNAL,
            signal_type: SignalType::Strong,
            from_direction: None,
        }
    } else {
        RedstoneSignal::none()
    }
}
