use std::collections::HashMap;

use bevy_ecs::prelude::*;
use valence_protocol::BlockPos;

/// Component storing an entity's memory.
#[derive(Component, Default)]
pub struct EntityMemory {
    /// Current path being followed.
    pub current_path: Vec<BlockPos>,
    /// Index into the current path.
    pub path_index: usize,
    /// Known positions of other entities.
    pub known_positions: HashMap<Entity, KnownEntityInfo>,
    /// Last known position of a target.
    pub last_target_pos: Option<BlockPos>,
    /// Current target entity (for combat, following, etc.).
    pub current_target: Option<Entity>,
    /// Home position (for villagers, pets, etc.).
    pub home_pos: Option<BlockPos>,
    /// Work position (for villagers).
    pub work_pos: Option<BlockPos>,
    /// General-purpose memory store.
    pub memories: HashMap<String, MemoryEntry>,
}

/// Information about a known entity.
#[derive(Clone, Debug)]
pub struct KnownEntityInfo {
    pub position: BlockPos,
    pub last_seen_tick: u64,
    pub relationship: EntityRelationship,
    pub threat_level: f32,
}

/// Relationship to another entity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityRelationship {
    /// No special relationship.
    Neutral,
    /// Friendly entity (villager of same type, etc.).
    Friendly,
    /// Hostile entity (attacking us, or we want to attack).
    Hostile,
    /// Target we are trying to reach.
    Target,
    /// Entity we are following.
    Followed,
}

/// A single memory entry.
#[derive(Clone, Debug)]
pub enum MemoryEntry {
    BlockPos(BlockPos),
    Entity(Entity),
    Float(f64),
    Integer(i64),
    Boolean(bool),
    String(String),
    Vec3(valence_math::DVec3),
}

impl Default for EntityMemory {
    fn default() -> Self {
        Self {
            current_path: Vec::new(),
            path_index: 0,
            known_positions: HashMap::new(),
            last_target_pos: None,
            current_target: None,
            home_pos: None,
            work_pos: None,
            memories: HashMap::new(),
        }
    }
}

impl EntityMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the next position in the current path.
    pub fn next_path_pos(&self) -> Option<BlockPos> {
        self.current_path.get(self.path_index).copied()
    }

    /// Advance to the next position in the path.
    pub fn advance_path(&mut self) {
        if self.path_index < self.current_path.len() {
            self.path_index += 1;
        }
    }

    /// Whether the path is exhausted.
    pub fn path_finished(&self) -> bool {
        self.path_index >= self.current_path.len()
    }

    /// Get remaining path positions.
    pub fn remaining_path(&self) -> &[BlockPos] {
        if self.path_index < self.current_path.len() {
            &self.current_path[self.path_index..]
        } else {
            &[]
        }
    }

    /// Clear the current path.
    pub fn clear_path(&mut self) {
        self.current_path.clear();
        self.path_index = 0;
    }

    /// Set a new path and reset the index.
    pub fn set_path(&mut self, path: Vec<BlockPos>) {
        self.current_path = path;
        self.path_index = 0;
    }

    /// Get the current target entity.
    pub fn target(&self) -> Option<Entity> {
        self.current_target
    }

    /// Set the current target.
    pub fn set_target(&mut self, target: Option<Entity>) {
        self.current_target = target;
    }

    /// Update knowledge about an entity.
    pub fn update_entity_knowledge(
        &mut self,
        entity: Entity,
        position: BlockPos,
        tick: u64,
        relationship: EntityRelationship,
    ) {
        self.known_positions.insert(
            entity,
            KnownEntityInfo {
                position,
                last_seen_tick: tick,
                relationship,
                threat_level: 0.0,
            },
        );
    }

    /// Forget an entity.
    pub fn forget_entity(&mut self, entity: &Entity) {
        self.known_positions.remove(entity);
    }

    /// Get all entities with a specific relationship.
    pub fn entities_with_relationship(
        &self,
        relationship: EntityRelationship,
    ) -> impl Iterator<Item = (Entity, &KnownEntityInfo)> {
        self.known_positions
            .iter()
            .filter(move |(_, info)| info.relationship == relationship)
            .map(|(e, info)| (*e, info))
    }

    /// Get the closest entity with a specific relationship.
    pub fn closest_entity(
        &self,
        from: BlockPos,
        relationship: EntityRelationship,
    ) -> Option<(Entity, BlockPos)> {
        self.known_positions
            .iter()
            .filter(|(_, info)| info.relationship == relationship)
            .min_by_key(|(_, info)| {
                let dx = (info.position.x - from.x).unsigned_abs();
                let dy = (info.position.y - from.y).unsigned_abs();
                let dz = (info.position.z - from.z).unsigned_abs();
                dx + dy + dz
            })
            .map(|(e, info)| (*e, info.position))
    }

    /// Check if we know about a specific entity.
    pub fn knows_entity(&self, entity: Entity) -> bool {
        self.known_positions.contains_key(&entity)
    }

    /// Clean up stale entity knowledge (older than max_age ticks).
    pub fn cleanup_stale(&mut self, current_tick: u64, max_age: u64) {
        self.known_positions
            .retain(|_, info| current_tick.saturating_sub(info.last_seen_tick) < max_age);
    }

    /// Get a typed memory value.
    pub fn get_memory(&self, key: &str) -> Option<&MemoryEntry> {
        self.memories.get(key)
    }

    /// Set a typed memory value.
    pub fn set_memory(&mut self, key: &str, value: MemoryEntry) {
        self.memories.insert(key.to_string(), value);
    }

    /// Remove a memory value.
    pub fn remove_memory(&mut self, key: &str) -> Option<MemoryEntry> {
        self.memories.remove(key)
    }

    /// Check if a boolean memory is true.
    pub fn get_bool(&self, key: &str) -> bool {
        matches!(self.memories.get(key), Some(MemoryEntry::Boolean(true)))
    }

    /// Set a boolean memory.
    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.memories
            .insert(key.to_string(), MemoryEntry::Boolean(value));
    }

    /// Get an integer memory.
    pub fn get_int(&self, key: &str) -> i64 {
        match self.memories.get(key) {
            Some(MemoryEntry::Integer(n)) => *n,
            _ => 0,
        }
    }

    /// Set an integer memory.
    pub fn set_int(&mut self, key: &str, value: i64) {
        self.memories
            .insert(key.to_string(), MemoryEntry::Integer(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_operations() {
        let mut mem = EntityMemory::new();
        mem.set_path(vec![
            BlockPos::new(0, 0, 0),
            BlockPos::new(1, 0, 0),
            BlockPos::new(2, 0, 0),
        ]);

        assert_eq!(mem.next_path_pos(), Some(BlockPos::new(0, 0, 0)));
        assert!(!mem.path_finished());

        mem.advance_path();
        assert_eq!(mem.next_path_pos(), Some(BlockPos::new(1, 0, 0)));

        mem.advance_path();
        assert_eq!(mem.next_path_pos(), Some(BlockPos::new(2, 0, 0)));

        mem.advance_path();
        assert!(mem.path_finished());
        assert_eq!(mem.next_path_pos(), None);
    }

    #[test]
    fn test_memory_operations() {
        let mut mem = EntityMemory::new();
        mem.set_bool("alert", true);
        assert!(mem.get_bool("alert"));

        mem.set_int("kill_count", 5);
        assert_eq!(mem.get_int("kill_count"), 5);

        mem.remove_memory("alert");
        assert!(!mem.get_bool("alert"));
    }
}
