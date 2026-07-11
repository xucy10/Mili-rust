pub mod behavior;
pub mod memory;
pub mod pathfinding;
pub mod perception;

use std::sync::Arc;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_entity::Position;
use valence_protocol::BlockPos;

pub use behavior::{BehaviorTree, BehaviorTreeBuilder};

/// Status of a behavior tree node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}
pub use memory::{EntityMemory, MemoryEntry};
pub use pathfinding::{find_path, PathfindingContext, PathfindingResult};
pub use perception::{DetectionResult, Perception};

/// Context passed to behavior tree nodes during evaluation.
pub struct BehaviorContext<'a> {
    pub entity: Entity,
    pub position: Option<BlockPos>,
    pub target: Option<Entity>,
    pub memory: &'a mut EntityMemory,
    pub current_tick: u64,
}

/// Trait for behavior tree nodes.
pub trait BehaviorNode: Send + Sync {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus;
}

/// Sequence node: runs children in order, fails on first failure.
pub struct Sequence {
    pub children: Vec<Arc<dyn BehaviorNode>>,
}

impl BehaviorNode for Sequence {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        for child in &self.children {
            match child.tick(entity, ctx) {
                BehaviorStatus::Failure => return BehaviorStatus::Failure,
                BehaviorStatus::Running => return BehaviorStatus::Running,
                BehaviorStatus::Success => continue,
            }
        }
        BehaviorStatus::Success
    }
}

/// Selector node: runs children in order, succeeds on first success.
pub struct Selector {
    pub children: Vec<Arc<dyn BehaviorNode>>,
}

impl BehaviorNode for Selector {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        for child in &self.children {
            match child.tick(entity, ctx) {
                BehaviorStatus::Success => return BehaviorStatus::Success,
                BehaviorStatus::Running => return BehaviorStatus::Running,
                BehaviorStatus::Failure => continue,
            }
        }
        BehaviorStatus::Failure
    }
}

/// Condition decorator: only runs child if condition is true.
pub struct Condition {
    pub condition: Arc<dyn Fn(&BehaviorContext) -> bool + Send + Sync>,
    pub child: Arc<dyn BehaviorNode>,
}

impl BehaviorNode for Condition {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if (self.condition)(ctx) {
            self.child.tick(entity, ctx)
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Plugin for entity AI systems.
pub struct EntityAiPlugin;

impl Plugin for EntityAiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (behavior_system, perception::perception_system).chain(),
        );
    }
}

fn behavior_system(
    mut query: Query<(Entity, &Position, &mut EntityMemory, &BehaviorTree)>,
) {
    for (entity, position, mut memory, tree) in &mut query {
        let mut ctx = BehaviorContext {
            entity,
            position: Some(BlockPos::new(
                position.0.x.floor() as i32,
                position.0.y.floor() as i32,
                position.0.z.floor() as i32,
            )),
            target: memory.current_target,
            memory: &mut memory,
            current_tick: 0,
        };

        tree.tick(entity, &mut ctx);
    }
}