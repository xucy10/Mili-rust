use bevy_ecs::prelude::*;
use valence_entity::Position;
use valence_server::client::Client;

#[derive(Component, Clone, Copy, Debug)]
pub struct Perception {
    pub sight_range: f64,
    pub hearing_range: f64,
}

impl Perception {
    pub fn new() -> Self {
        Self {
            sight_range: 16.0,
            hearing_range: 16.0,
        }
    }

    pub fn with_ranges(sight: f64, hearing: f64) -> Self {
        Self {
            sight_range: sight,
            hearing_range: hearing,
        }
    }
}

impl Default for Perception {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
pub struct DetectionResult {
    pub detected_entity: Entity,
    pub distance: f64,
    pub is_visible: bool,
}

pub fn perception_system(
    mob_query: Query<(Entity, &Position, &Perception)>,
    target_query: Query<(Entity, &Position), With<Client>>,
) {
    for (_mob_entity, mob_pos, perception) in &mob_query {
        let mut _nearest: Option<DetectionResult> = None;

        for (target_entity, target_pos) in &target_query {
            let dist = (mob_pos.0 - target_pos.0).length();

            if dist <= perception.sight_range {
                _nearest = Some(DetectionResult {
                    detected_entity: target_entity,
                    distance: dist,
                    is_visible: true,
                });
            }
        }
    }
}