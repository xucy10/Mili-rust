use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::{
    get_direction_offset, get_opposite_direction, get_power_level, offset_pos, RedstoneSignal,
    RedstoneStrength, SignalType, MAX_SIGNAL,
};

#[derive(Component, Debug)]
pub struct RedstoneRepeater {
    pub delay: u8,
    pub locked: bool,
    pub powered: bool,
    pub facing: Direction,
    pub input_power: RedstoneStrength,
    pub output_power: RedstoneStrength,
    pub locked_input_power: RedstoneStrength,
    pub tick_count: u8,
    pub last_tick: u64,
}

impl Default for RedstoneRepeater {
    fn default() -> Self {
        Self {
            delay: 1,
            locked: false,
            powered: false,
            facing: Direction::North,
            input_power: 0,
            output_power: 0,
            locked_input_power: 0,
            tick_count: 0,
            last_tick: 0,
        }
    }
}

impl RedstoneRepeater {
    pub fn new(facing: Direction) -> Self {
        Self {
            facing,
            ..Default::default()
        }
    }

    pub fn with_delay(mut self, delay: u8) -> Self {
        self.delay = delay.clamp(1, 4);
        self
    }

    pub fn get_input_direction(&self) -> Direction {
        get_opposite_direction(self.facing)
    }

    pub fn get_output_direction(&self) -> Direction {
        self.facing
    }

    pub fn get_lock_directions(&self) -> [Direction; 2] {
        match self.facing {
            Direction::North | Direction::South => [Direction::West, Direction::East],
            Direction::West | Direction::East => [Direction::North, Direction::South],
            _ => [Direction::North, Direction::South],
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn get_output_signal(&self) -> RedstoneStrength {
        if self.powered {
            MAX_SIGNAL
        } else {
            0
        }
    }

    pub fn get_output_for_direction(&self, dir: Direction) -> RedstoneStrength {
        if dir == self.facing && self.powered {
            MAX_SIGNAL
        } else {
            0
        }
    }

    pub fn calculate_delay_ticks(&self) -> u32 {
        self.delay as u32 * 2
    }

    pub fn update_signal(&mut self, inputs: &[RedstoneSignal], current_tick: u64) -> bool {
        let input_dir = self.get_input_direction();
        let input_signal = inputs
            .iter()
            .find(|s| s.from_direction == Some(input_dir))
            .map(|s| s.strength)
            .unwrap_or(0);

        let lock_signals: Vec<RedstoneStrength> = self
            .get_lock_directions()
            .iter()
            .filter_map(|&dir| {
                inputs
                    .iter()
                    .find(|s| s.from_direction == Some(dir))
                    .map(|s| s.strength)
            })
            .collect();

        let is_locked_by_signal = lock_signals.iter().any(|&s| s > 0);

        if is_locked_by_signal {
            self.locked = true;
            self.locked_input_power = input_signal;
            return false;
        }

        if self.locked {
            self.locked = false;
        }

        self.input_power = input_signal;

        let ticks_since_last = current_tick.wrapping_sub(self.last_tick);
        self.tick_count += ticks_since_last as u8;

        let target_power = if self.input_power > 0 { MAX_SIGNAL } else { 0 };

        if self.powered != (target_power > 0) {
            if self.tick_count >= self.calculate_delay_ticks() as u8 {
                self.powered = target_power > 0;
                self.output_power = if self.powered { MAX_SIGNAL } else { 0 };
                self.last_tick = current_tick;
                self.tick_count = 0;
                return true;
            }
        } else if self.input_power == 0 && self.powered {
            if self.tick_count >= self.calculate_delay_ticks() as u8 {
                self.powered = false;
                self.output_power = 0;
                self.last_tick = current_tick;
                self.tick_count = 0;
                return true;
            }
        }

        false
    }

    pub fn needs_scheduled_tick(&self) -> bool {
        let target_power = if self.input_power > 0 { MAX_SIGNAL } else { 0 };

        self.powered != (target_power > 0)
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
                _ => PropValue::North,
            },
        );

        state = state.set(
            PropName::Delay,
            PropValue::from_u16(self.delay as u16).unwrap_or(PropValue::One),
        );

        state = state.set(
            PropName::Locked,
            if self.locked {
                PropValue::True
            } else {
                PropValue::False
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
        if state.to_kind() != BlockKind::Repeater {
            return None;
        }

        let facing = state.get(PropName::Facing).and_then(|v| match v {
            PropValue::North => Some(Direction::North),
            PropValue::South => Some(Direction::South),
            PropValue::West => Some(Direction::West),
            PropValue::East => Some(Direction::East),
            _ => None,
        })?;

        let delay = state
            .get(PropName::Delay)
            .and_then(|v| v.to_u16())
            .unwrap_or(1) as u8;

        let locked = state
            .get(PropName::Locked)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        let powered = state
            .get(PropName::Powered)
            .and_then(|v| v.to_bool())
            .unwrap_or(false);

        Some(Self {
            delay,
            locked,
            powered,
            facing,
            input_power: 0,
            output_power: if powered { MAX_SIGNAL } else { 0 },
            locked_input_power: 0,
            tick_count: 0,
            last_tick: 0,
        })
    }
}

pub fn is_repeater(state: BlockState) -> bool {
    state.to_kind() == BlockKind::Repeater
}

pub fn get_repeater_signal(pos: BlockPos, state: BlockState) -> RedstoneSignal {
    if !is_repeater(state) {
        return RedstoneSignal::none();
    }

    let repeater = RedstoneRepeater::from_block_state(state).unwrap_or_default();

    if repeater.powered {
        RedstoneSignal {
            strength: MAX_SIGNAL,
            signal_type: SignalType::Strong,
            from_direction: None,
        }
    } else {
        RedstoneSignal::none()
    }
}
