use bevy_ecs::prelude::*;
use glam::DVec3;
use valence_entity::Position;
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

    /// Create a perception for a hostile mob (good sight, medium hearing).
    pub fn hostile() -> Self {
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

    /// Create a perception for a passive mob (medium sight, good hearing).
    pub fn passive() -> Self {
        Self {
            sight_range: 12.0,
            hearing_range: 16.0,
            fov: 180.0,
            night_vision: false,
            hearing_through_walls: false,
            smell_range: 0.0,
            focused_entity: None,
        }
    }

    /// Create a perception for an enderman (long sight, no hearing).
    pub fn enderman() -> Self {
        Self {
            sight_range: 64.0,
            hearing_range: 0.0,
            fov: 360.0,
            night_vision: true,
            hearing_through_walls: false,
            smell_range: 0.0,
            focused_entity: None,
        }
    }

    /// Create a perception for a bat (short sight, good hearing).
    pub fn bat() -> Self {
        Self {
            sight_range: 8.0,
            hearing_range: 24.0,
            fov: 360.0,
            night_vision: true,
            hearing_through_walls: true,
            smell_range: 0.0,
            focused_entity: None,
        }
    }

    /// Check if a position is within sight range.
    pub fn can_see(&self, from: DVec3, to: DVec3) -> bool {
        let diff = to - from;
        let distance = diff.length();
        distance <= self.sight_range as f64
    }

    /// Check if a position is within sight range and field of view.
    pub fn can_see_with_fov(&self, from: DVec3, to: DVec3, facing_dir: DVec3) -> bool {
        let diff = to - from;
        let distance = diff.length();

        if distance > self.sight_range as f64 {
            return false;
        }

        // If 360 degree FOV, just check range
        if self.fov >= 360.0 {
            return true;
        }

        // Check angle
        if distance < 0.001 {
            return true; // Same position
        }

        let to_target = diff.normalize();
        let facing = facing_dir.normalize();

        let dot = to_target.dot(facing);
        let angle = dot.acos().to_degrees();

        angle <= self.fov / 2.0
    }

    /// Check if a position is within hearing range.
    pub fn can_hear(&self, from: DVec3, to: DVec3) -> bool {
        let diff = to - from;
        let distance = diff.length();
        distance <= self.hearing_range as f64
    }

    /// Check if a position is within smell range.
    pub fn can_smell(&self, from: DVec3, to: DVec3) -> bool {
        if self.smell_range <= 0.0 {
            return false;
        }
        let diff = to - from;
        let distance = diff.length();
        distance <= self.smell_range as f64
    }

    /// Check if an entity can detect another entity.
    pub fn can_detect(
        &self,
        from_pos: DVec3,
        from_facing: DVec3,
        target_pos: DVec3,
        target_relationship: EntityRelationship,
    ) -> DetectionResult {
        // Check sight
        let can_see = self.can_see_with_fov(from_pos, target_pos, from_facing);

        // Check hearing
        let can_hear = self.can_hear(from_pos, target_pos);

        // Check smell
        let can_smell = self.can_smell(from_pos, target_pos);

        let detected = can_see || can_hear || can_smell;

        DetectionResult {
            detected,
            can_see,
            can_hear,
            can_smell,
            distance: from_pos.distance(target_pos) as f32,
            confidence: if can_see {
                1.0
            } else if can_hear {
                0.7
            } else if can_smell {
                0.3
            } else {
                0.0
            },
        }
    }
}

/// Result of a detection check.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Whether the entity was detected.
    pub detected: bool,
    /// Whether detected by sight.
    pub can_see: bool,
    /// Whether detected by hearing.
    pub can_hear: bool,
    /// Whether detected by smell.
    pub can_smell: bool,
    /// Distance to the target.
    pub distance: f32,
    /// Confidence in the detection (0.0 to 1.0).
    pub confidence: f32,
}

/// Perception system that updates entity memory based on what they can see/hear.
pub fn perception_system(
    mut query: Query<(Entity, &Position, &mut Perception, &mut EntityMemory)>,
    all_entities: Query<(Entity, &Position)>,
    time: Res<Time>,
) {
    let current_tick = time.tick().as_secs() * 20; // Approximate tick count

    for (entity, pos, mut perception, mut memory) in &mut query {
        let from_pos = pos.0;

        // Get facing direction from velocity or default
        let facing = if memory.last_target_pos.is_some() {
            // If we have a target, face towards it
            let target_pos = memory.last_target_pos.unwrap();
            let diff = DVec3::new(
                target_pos.x as f64 - from_pos.x,
                target_pos.y as f64 - from_pos.y,
                target_pos.z as f64 - from_pos.z,
            );
            if diff.length() > 0.001 {
                diff.normalize()
            } else {
                DVec3::new(0.0, 0.0, 1.0) // Default facing south
            }
        } else {
            DVec3::new(0.0, 0.0, 1.0) // Default facing south
        };

        // Check all other entities
        for (other_entity, other_pos) in &all_entities {
            if other_entity == entity {
                continue;
            }

            let result =
                perception.can_detect(from_pos, facing, other_pos.0, EntityRelationship::Neutral);

            if result.detected {
                let block_pos = BlockPos::new(
                    other_pos.0.x.floor() as i32,
                    other_pos.0.y.floor() as i32,
                    other_pos.0.z.floor() as i32,
                );

                // Update memory with detected entity
                memory.update_entity_knowledge(
                    other_entity,
                    block_pos,
                    current_tick as u64,
                    EntityRelationship::Neutral,
                );
            }
        }

        // Clean up stale entity knowledge (entities not seen for 5 seconds)
        memory.cleanup_stale(current_tick as u64, 100);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sight_range() {
        let perception = Perception {
            sight_range: 10.0,
            ..Default::default()
        };

        assert!(perception.can_see(DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0)));
        assert!(perception.can_see(DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0)));
        assert!(!perception.can_see(DVec3::ZERO, DVec3::new(15.0, 0.0, 0.0)));
    }

    #[test]
    fn test_fov() {
        let perception = Perception {
            sight_range: 100.0,
            fov: 90.0,
            ..Default::default()
        };

        // Looking east (1,0,0)
        let facing = DVec3::new(1.0, 0.0, 0.0);

        // Directly in front
        assert!(perception.can_see_with_fov(DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0), facing));

        // 45 degrees to the side (should be visible with 90 degree FOV)
        assert!(perception.can_see_with_fov(DVec3::ZERO, DVec3::new(10.0, 0.0, 10.0), facing));

        // Directly to the side (90 degrees - at the edge)
        let result = perception.can_see_with_fov(DVec3::ZERO, DVec3::new(0.0, 0.0, 10.0), facing);
        // This should be at the edge of the FOV

        // Behind (should not be visible)
        assert!(!perception.can_see_with_fov(DVec3::ZERO, DVec3::new(-10.0, 0.0, 0.0), facing));
    }

    #[test]
    fn test_hearing_range() {
        let perception = Perception {
            hearing_range: 8.0,
            ..Default::default()
        };

        assert!(perception.can_hear(DVec3::ZERO, DVec3::new(5.0, 0.0, 0.0)));
        assert!(!perception.can_hear(DVec3::ZERO, DVec3::new(10.0, 0.0, 0.0)));
    }
}
