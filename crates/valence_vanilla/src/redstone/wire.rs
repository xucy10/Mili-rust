use bevy_ecs::prelude::*;
use valence_generated::block::{BlockKind, BlockState, PropName, PropValue};
use valence_protocol::{BlockPos, Direction};

use super::signal::{
    get_horizontal_directions, get_opposite_direction, get_power_level, is_redstone_conductor,
    offset_pos, RedstoneStrength, MAX_SIGNAL,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireConnection {
    None,
    Side,
    Up,
}

impl WireConnection {
    pub fn from_prop_value(val: PropValue) -> Self {
        match val {
            PropValue::Up => WireConnection::Up,
            PropValue::Side => WireConnection::Side,
            _ => WireConnection::None,
        }
    }

    pub fn to_prop_value(self) -> PropValue {
        match self {
            WireConnection::Up => PropValue::Up,
            WireConnection::Side => PropValue::Side,
            WireConnection::None => PropValue::None,
        }
    }
}

#[derive(Component, Debug)]
pub struct RedstoneWire {
    pub power: RedstoneStrength,
    pub connections: [WireConnection; 4],
    pub updating: bool,
}

impl Default for RedstoneWire {
    fn default() -> Self {
        Self {
            power: 0,
            connections: [WireConnection::None; 4],
            updating: false,
        }
    }
}

impl RedstoneWire {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_power(mut self, power: RedstoneStrength) -> Self {
        self.power = power.min(MAX_SIGNAL);
        self
    }

    pub fn get_connection(&self, dir: Direction) -> WireConnection {
        match dir {
            Direction::North => self.connections[0],
            Direction::South => self.connections[1],
            Direction::West => self.connections[2],
            Direction::East => self.connections[3],
            _ => WireConnection::None,
        }
    }

    pub fn set_connection(&mut self, dir: Direction, conn: WireConnection) {
        match dir {
            Direction::North => self.connections[0] = conn,
            Direction::South => self.connections[1] = conn,
            Direction::West => self.connections[2] = conn,
            Direction::East => self.connections[3] = conn,
            _ => {}
        }
    }

    pub fn get_prop_name_for_direction(dir: Direction) -> PropName {
        match dir {
            Direction::North => PropName::North,
            Direction::South => PropName::South,
            Direction::West => PropName::West,
            Direction::East => PropName::East,
            _ => PropName::North,
        }
    }

    pub fn update_connections(&mut self, state: BlockState) {
        for dir in get_horizontal_directions() {
            let prop = Self::get_prop_name_for_direction(dir);
            if let Some(val) = state.get(prop) {
                self.set_connection(dir, WireConnection::from_prop_value(val));
            }
        }
    }

    pub fn to_block_state(&self) -> BlockState {
        let mut state = BlockState::REDSTONE_WIRE;

        state = state.set(
            PropName::Power,
            PropValue::from_u16(self.power as u16).unwrap_or(PropValue::None),
        );

        for dir in get_horizontal_directions() {
            let prop = Self::get_prop_name_for_direction(dir);
            let conn = self.get_connection(dir);
            state = state.set(prop, conn.to_prop_value());
        }

        state
    }

    pub fn calculate_connections(
        &self,
        pos: BlockPos,
        get_block: impl Fn(BlockPos) -> BlockState,
    ) -> [WireConnection; 4] {
        let mut connections = [WireConnection::None; 4];

        for (i, dir) in get_horizontal_directions().iter().enumerate() {
            let neighbor_pos = offset_pos(pos, *dir);
            let neighbor_state = get_block(neighbor_pos);

            connections[i] =
                self.calculate_connection_for_direction(*dir, neighbor_state, get_block);
        }

        connections
    }

    fn calculate_connection_for_direction(
        &self,
        dir: Direction,
        neighbor_state: BlockState,
        get_block: impl Fn(BlockPos) -> BlockState,
    ) -> WireConnection {
        if is_wire(neighbor_state) {
            return WireConnection::Side;
        }

        if is_redstone_conductor(neighbor_state) {
            let above_pos = offset_pos(offset_pos(BlockPos::new(0, 0, 0), dir), Direction::Up);
            let above_state = get_block(above_pos);
            if is_redstone_conductor(above_state) {
                return WireConnection::Up;
            }
            return WireConnection::Side;
        }

        WireConnection::None
    }

    pub fn update_signal(&mut self, inputs: &[RedstoneSignal]) -> bool {
        let max_input = inputs
            .iter()
            .filter(|s| s.strength > 0)
            .map(|s| s.strength)
            .max()
            .unwrap_or(0);

        let new_power = max_input.saturating_sub(1);

        if self.power != new_power {
            self.power = new_power;
            true
        } else {
            false
        }
    }

    pub fn get_signal_strength_for_direction(&self, dir: Direction) -> RedstoneStrength {
        let conn = self.get_connection(dir);
        match conn {
            WireConnection::Side => self.power,
            WireConnection::Up => self.power,
            WireConnection::None => 0,
        }
    }
}

pub fn is_wire(state: BlockState) -> bool {
    state.to_kind() == BlockKind::RedstoneWire
}

pub fn get_wire_signal(pos: BlockPos, state: BlockState) -> RedstoneSignal {
    if !is_wire(state) {
        return RedstoneSignal::none();
    }

    let power = get_power_level(state);
    RedstoneSignal {
        strength: power,
        signal_type: SignalType::Weak,
        from_direction: None,
    }
}
