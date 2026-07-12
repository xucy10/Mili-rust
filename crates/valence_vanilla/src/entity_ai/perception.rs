use bevy_ecs::prelude::*;
use valence_entity::Position;
use valence_math::DVec3;
use valence_protocol::BlockPos;

use super::memory::{EntityMemory, EntityRelationship};

/// Component for entity perception (sight, hearing, smell).
#[derive(Component)]
pub struct Perception {
    /// Sight range in blocks.
    pub sight_range: f32,
    /// Hearing range in blocks.
    pub hearing_range: f32,
    /// Field of view in degrees (total angle, centered on facing direction).
    pub fov: f32,
    /// Whether the entity can see in the dark.
    pub night_vision: bool,
    /// Whether the entity can hear through walls.
    pub hearing_through_walls: bool,
    /// Smell range (for tracking entities by scent).
    pub smell_range: f32,
    /// Target the entity is currently focused on.
    pub focused_entity: Option<Entity>,
}

impl Default for Perception {
    fn default() -> Self {
        Self {
            sight_range: 16.0,
            hearing_range: 16.0,
            fov: 110.0,
            night_vision: false,
            hearing_through_walls: false,
            smell_range: 0.0,
            focused_entity: None,
        }
    }
}

impl Perception {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a perception with custom sight and hearing ranges.
    pub fn with_ranges(sight: f32, hearing: f32) -> Self {
        Self {
            sight_range: sight,
            hearing_range: hearing,
            ..Default::default()
        }
    }
}

/// Result of a perception check.
#[derive(Clone, Debug)]
pub struct DetectionResult {
    pub entity: Entity,
    pub detected_by_sight: bool,
    pub detected_by_sound: bool,
    pub detected_by_smell: bool,
    pub distance: f32,
}

/// System that updates entity perception each tick.
pub fn perception_system(
    mut perceivers: Query<(Entity, &Position, &Perception, &mut EntityMemory)>,
    targets: Query<(Entity, &Position)>,
) {
    for (perceiver_entity, perceiver_pos, perception, mut memory) in &mut perceivers {
        for (target_entity, target_pos) in &targets {
            if perceiver_entity == target_entity {
                continue;
            }

            let distance = (target_pos.0 - perceiver_pos.0).length() as f32;

            // Check sight
            let can_see = distance <= perception.sight_range;

            // Check hearing
            let can_hear = distance <= perception.hearing_range;

            if can_see || can_hear {
                let result = DetectionResult {
                    entity: target_entity,
                    detected_by_sight: can_see,
                    detected_by_sound: can_hear,
                    detected_by_smell: false,
                    distance,
                };

                // Update memory with detected entity
                memory.known_positions.insert(
                    target_entity,
                    super::memory::KnownEntityInfo {
                        position: BlockPos::new(
                            target_pos.0.x as i32,
                            target_pos.0.y as i32,
                            target_pos.0.z as i32,
                        ),
                        last_seen_tick: 0, // TODO: use actual tick
                        relationship: EntityRelationship::Neutral,
                        threat_level: 0.0,
                    },
                );

                if can_see {
                    memory.last_target_pos = Some(BlockPos::new(
                        target_pos.0.x as i32,
                        target_pos.0.y as i32,
                        target_pos.0.z as i32,
                    ));
                }
            }
        }
    }
}
