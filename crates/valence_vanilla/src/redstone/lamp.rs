use valence_generated::block::{BlockState, PropName, PropValue};

pub fn process_lamp_update(state: BlockState, powered: bool) -> BlockState {
    state.set(
        PropName::Lit,
        if powered {
            PropValue::True
        } else {
            PropValue::False
        },
    )
}