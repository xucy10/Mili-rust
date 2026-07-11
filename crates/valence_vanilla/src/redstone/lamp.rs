use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::RedstoneStrength;

#[derive(Component, Debug)]
pub struct RedstoneLamp {
    pub lit: bool,
    pub power: RedstoneStrength,
    pub turn_on_tick: u64,
    pub turn_off_tick: u64,
}

impl Default for RedstoneLamp {
    fn default() -> Self {
        Self {
            lit: false,
            power: 0,
            turn_on_tick: 0,
            turn_off_tick: 0,
        }
    }
}

impl RedstoneLamp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn powered() -> Self {
        Self {
            lit: true,
            ..Default::default()
        }
    }

    pub fn is_lit(&self) -> bool {
        self.lit
    }

    pub fn set_lit(&mut self, lit: bool, current_tick: u64) {
        if lit && !self.lit {
            self.turn_on_tick = current_tick;
        } else if !lit && self.lit {
            self.turn_off_tick = current_tick;
        }
        self.lit = lit;
    }

    pub fn get_luminance(&self) -> u8 {
        if self.lit {
            15
        } else {
            0
        }
    }

    pub fn update_signal(&mut self, power: RedstoneStrength, current_tick: u64) -> bool {
        let was_lit = self.lit;
        self.power = power;

        if power > 0 {
            self.set_lit(true, current_tick);
        } else {
            self.set_lit(false, current_tick);
        }

        was_lit != self.lit
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = BlockState::REDSTONE_LAMP;

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

    pub fn from_block_state(state: BlockState) -> Option<Self> {
        if state.to_kind() != BlockKind::RedstoneLamp {
            return None;
        }

        let lit = state
            .get(PropName::Lit)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            lit,
            power: if lit { 15 } else { 0 },
            turn_on_tick: 0,
            turn_off_tick: 0,
        })
    }
}

pub fn is_lamp(state: BlockState) -> bool {
    state.to_kind() == BlockKind::RedstoneLamp
}

pub fn get_lamp_power(state: BlockState) -> RedstoneStrength {
    if !is_lamp(state) {
        return 0;
    }

    if let Some(lit) = state.get(PropName::Lit) {
        if lit.to_bool().unwrap_or(false) {
            15
        } else {
            0
        }
    } else {
        0
    }
}
