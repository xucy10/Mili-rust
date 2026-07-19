use valence_generated::block::{BlockState, PropName, PropValue};

use super::signal::{get_power_level, RedstoneStrength};

pub fn calculate_wire_power(
    state: BlockState,
    neighbor_powers: [RedstoneStrength; 4],
) -> Option<BlockState> {
    let max_power = neighbor_powers.iter().copied().max().unwrap_or(0);
    let new_power = max_power.saturating_sub(1);
    let old_power = get_power_level(state);

    if new_power != old_power {
        Some(
            state.set(
                PropName::Power,
                PropValue::from_u16(new_power as u16).unwrap_or(PropValue::_0),
            ),
        )
    } else {
        None
    }
}