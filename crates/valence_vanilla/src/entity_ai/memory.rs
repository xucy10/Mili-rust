use std::collections::HashMap;

use bevy_ecs::prelude::*;
use valence_protocol::BlockPos;

/// Component storing an entity's memory.
#[derive(Component)]
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
        &self.current_path[self.path_index..]
    }

    /// Clear the current path.
    pub fn clear_path(&mut self) {
        self.current_path.clear();
        self.path_index = 0;
    }

    /// Set a new path.
    pub fn set_path(&mut self, path: Vec<BlockPos>) {
        self.current_path = path;
        self.path_index = 0;
    }
}
