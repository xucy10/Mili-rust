use std::collections::HashMap;

use bevy_ecs::prelude::*;

#[derive(Clone, Debug)]
pub struct MemoryEntry {
    pub value: MemoryValue,
    pub timestamp: u64,
}

#[derive(Clone, Debug)]
pub enum MemoryValue {
    Entity(Entity),
    Position(valence_protocol::BlockPos),
    Float(f64),
    Bool(bool),
}

#[derive(Component, Debug, Clone)]
pub struct EntityMemory {
    entries: HashMap<String, MemoryEntry>,
    pub current_target: Option<Entity>,
    pub home_position: Option<valence_protocol::BlockPos>,
    pub last_damage_source: Option<String>,
    pub last_seen_player: Option<Entity>,
    pub panic_target: Option<valence_protocol::BlockPos>,
}

impl EntityMemory {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            current_target: None,
            home_position: None,
            last_damage_source: None,
            last_seen_player: None,
            panic_target: None,
        }
    }

    pub fn remember(&mut self, key: &str, value: MemoryValue, tick: u64) {
        self.entries.insert(
            key.to_string(),
            MemoryEntry {
                value,
                timestamp: tick,
            },
        );
    }

    pub fn recall(&self, key: &str) -> Option<&MemoryEntry> {
        self.entries.get(key)
    }

    pub fn forget(&mut self, key: &str) {
        self.entries.remove(key);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_target = None;
    }

    pub fn is_expired(&self, key: &str, current_tick: u64, max_age: u64) -> bool {
        match self.entries.get(key) {
            Some(entry) => current_tick - entry.timestamp > max_age,
            None => true,
        }
    }
}