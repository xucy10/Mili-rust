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
        app.add_systems(Update, (
            behavior_system,
            perception::perception_system,
        ).chain());
    }
}

/// System that ticks all behavior trees.
pub fn behavior_system(
    mut query: Query<(Entity, &Position, &BehaviorTree, &mut EntityMemory)>,
    time: Res<Time>,
) {
    let current_tick = time.tick().as_secs() * 20;

    for (entity, pos, behavior_tree, mut memory) in &mut query {
        let block_pos = BlockPos::new(
            pos.0.x.floor() as i32,
            pos.0.y.floor() as i32,
            pos.0.z.floor() as i32,
        );

        let mut ctx = BehaviorContext {
            entity,
            position: Some(block_pos),
            target: memory.current_target,
            memory: &mut memory,
            current_tick,
        };

        behavior_tree.tick(entity, &mut ctx);
    }
}

/// Helper to create a simple patrol behavior.
pub fn patrol_behavior(waypoints: Vec<BlockPos>) -> Arc<dyn BehaviorNode> {
    let mut nodes: Vec<Arc<dyn BehaviorNode>> = Vec::new();

    // Set initial target
    if let Some(first) = waypoints.first() {
        nodes.push(Arc::new(behavior::SetMemory {
            key: "patrol_index".to_string(),
            value: MemoryEntry::Integer(0),
        }));
        nodes.push(Arc::new(behavior::SetMemory {
            key: "patrol_target".to_string(),
            value: MemoryEntry::BlockPos(*first),
        }));
    }

    // Create the patrol loop
    let move_node: Arc<dyn BehaviorNode> = Arc::new(behavior::MoveToPosition);
    let wait_node: Arc<dyn BehaviorNode> = Arc::new(behavior::Wait { ticks: 40 }); // Wait 2 seconds

    nodes.push(move_node);
    nodes.push(wait_node);

    // Advance to next waypoint (would need custom action node in production)
    // For now, just use a simple sequence
    Arc::new(Sequence { children: nodes })
}

/// Helper to create a follow entity behavior.
pub fn follow_behavior(target: Entity) -> Arc<dyn BehaviorNode> {
    let nodes: Vec<Arc<dyn BehaviorNode>> = vec![
        Arc::new(behavior::SetMemory {
            key: "follow_target".to_string(),
            value: MemoryEntry::Entity(target),
        }),
        Arc::new(behavior::MoveToPosition),
        Arc::new(behavior::Wait { ticks: 5 }), // Small delay between checks
    ];

    Arc::new(Sequence { children: nodes })
}

/// Helper to create an idle behavior with random wandering.
pub fn idle_wander_behavior() -> Arc<dyn BehaviorNode> {
    let nodes: Vec<Arc<dyn BehaviorNode>> = vec![
        Arc::new(behavior::Wait {
            ticks: 40 + rand::random::<u32>() % 80,
        }),
        Arc::new(behavior::MoveToPosition),
    ];

    Arc::new(Sequence { children: nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountAction {
        count: Arc<AtomicU32>,
    }

    impl BehaviorNode for CountAction {
        fn tick(&self, _entity: Entity, _ctx: &mut BehaviorContext) -> BehaviorStatus {
            self.count.fetch_add(1, Ordering::SeqCst);
            BehaviorStatus::Success
        }
    }

    #[test]
    fn test_sequence_all_success() {
        let count = Arc::new(AtomicU32::new(0));
        let seq = Sequence {
            children: vec![
                Arc::new(CountAction {
                    count: count.clone(),
                }),
                Arc::new(CountAction {
                    count: count.clone(),
                }),
            ],
        };

        let mut mem = EntityMemory::new();
        let mut ctx = BehaviorContext {
            entity: Entity::from_raw(0),
            position: None,
            target: None,
            memory: &mut mem,
            current_tick: 0,
        };

        let status = seq.tick(Entity::from_raw(0), &mut ctx);
        assert_eq!(status, BehaviorStatus::Success);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_selector_first_success() {
        struct FailAction;
        impl BehaviorNode for FailAction {
            fn tick(&self, _: Entity, _: &mut BehaviorContext) -> BehaviorStatus {
                BehaviorStatus::Failure
            }
        }

        let count = Arc::new(AtomicU32::new(0));
        let sel = Selector {
            children: vec![
                Arc::new(FailAction),
                Arc::new(CountAction {
                    count: count.clone(),
                }),
            ],
        };

        let mut mem = EntityMemory::new();
        let mut ctx = BehaviorContext {
            entity: Entity::from_raw(0),
            position: None,
            target: None,
            memory: &mut mem,
            current_tick: 0,
        };

        let status = sel.tick(Entity::from_raw(0), &mut ctx);
        assert_eq!(status, BehaviorStatus::Success);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
