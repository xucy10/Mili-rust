use std::sync::Arc;

use bevy_ecs::prelude::*;
use valence_protocol::BlockPos;

use super::memory::MemoryEntry;
use super::{BehaviorContext, BehaviorNode, BehaviorStatus};

/// Component for entity behavior trees.
#[derive(Component)]
pub struct BehaviorTree {
    pub root: Option<Arc<dyn BehaviorNode>>,
}

impl Default for BehaviorTree {
    fn default() -> Self {
        Self { root: None }
    }
}

impl BehaviorTree {
    /// Tick the behavior tree for an entity.
    pub fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if let Some(root) = &self.root {
            root.tick(entity, ctx)
        } else {
            BehaviorStatus::Failure
        }
    }
}


/// Helper to build behavior trees.
pub struct BehaviorTreeBuilder {
    children: Vec<Arc<dyn BehaviorNode>>,
}

impl BehaviorTreeBuilder {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }

    /// Add a child node to the sequence.
    pub fn sequence(mut self, children: Vec<Arc<dyn BehaviorNode>>) -> Self {
        self.children.push(Arc::new(super::Sequence { children }));
        self
    }

    /// Add a child node to the selector.
    pub fn selector(mut self, children: Vec<Arc<dyn BehaviorNode>>) -> Self {
        self.children.push(Arc::new(super::Selector { children }));
        self
    }

    /// Add a leaf action node.
    pub fn action(mut self, action: Arc<dyn ActionNode>) -> Self {
        self.children.push(action);
        self
    }

    /// Add a condition check.
    pub fn condition(
        mut self,
        check: Arc<dyn Fn(&BehaviorContext) -> bool + Send + Sync>,
        child: Arc<dyn BehaviorNode>,
    ) -> Self {
        self.children.push(Arc::new(super::Condition {
            condition: check,
            child,
        }));
        self
    }

    /// Build the behavior tree.
    pub fn build(self) -> BehaviorTree {
        let root = if self.children.len() == 1 {
            self.children.into_iter().next().unwrap()
        } else {
            Arc::new(super::Sequence {
                children: self.children,
            })
        };
        BehaviorTree { root: Some(root) }
    }
}

/// A leaf action node that performs some action.
pub trait ActionNode: Send + Sync {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus;
}

// ActionNode implements BehaviorNode via a blanket impl
impl<T: ActionNode> BehaviorNode for T {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        ActionNode::tick(self, entity, ctx)
    }
}

/// Simple action node that wraps a closure.
pub struct SimpleAction {
    pub func: Box<dyn Fn(Entity, &mut BehaviorContext) -> BehaviorStatus + Send + Sync>,
}

impl ActionNode for SimpleAction {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        (self.func)(entity, ctx)
    }
}

/// Node that repeats its child a fixed number of times.
pub struct Repeat {
    pub count: u32,
    pub child: Arc<dyn BehaviorNode>,
}

impl BehaviorNode for Repeat {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        for _ in 0..self.count {
            match self.child.tick(entity, ctx) {
                BehaviorStatus::Failure => return BehaviorStatus::Failure,
                BehaviorStatus::Running => return BehaviorStatus::Running,
                BehaviorStatus::Success => continue,
            }
        }
        BehaviorStatus::Success
    }
}

/// Node that inverts the result of its child.
pub struct Inverter {
    pub child: Arc<dyn BehaviorNode>,
}

impl BehaviorNode for Inverter {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        match self.child.tick(entity, ctx) {
            BehaviorStatus::Success => BehaviorStatus::Failure,
            BehaviorStatus::Failure => BehaviorStatus::Success,
            BehaviorStatus::Running => BehaviorStatus::Running,
        }
    }
}

/// Node that runs children until one fails or is running.
/// Used for continuous sequences.
pub struct Parallel {
    pub children: Vec<Arc<dyn BehaviorNode>>,
    /// How many children must succeed for the parallel to succeed.
    pub required_successes: usize,
}

impl BehaviorNode for Parallel {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        let mut successes = 0;
        let mut any_running = false;

        for child in &self.children {
            match child.tick(entity, ctx) {
                BehaviorStatus::Success => successes += 1,
                BehaviorStatus::Running => any_running = true,
                BehaviorStatus::Failure => return BehaviorStatus::Failure,
            }
        }

        if successes >= self.required_successes {
            BehaviorStatus::Success
        } else if any_running {
            BehaviorStatus::Running
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Node that randomly selects and runs one of its children.
pub struct RandomSelector {
    pub children: Vec<Arc<dyn BehaviorNode>>,
}

impl BehaviorNode for RandomSelector {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let mut rng = thread_rng();
        let indices: Vec<usize> = (0..self.children.len()).collect();
        let shuffled = {
            let mut idx = indices;
            idx.shuffle(&mut rng);
            idx
        };

        for i in shuffled {
            match self.children[i].tick(entity, ctx) {
                BehaviorStatus::Success => return BehaviorStatus::Success,
                BehaviorStatus::Running => return BehaviorStatus::Running,
                BehaviorStatus::Failure => continue,
            }
        }

        BehaviorStatus::Failure
    }
}

/// Node that runs its child once and caches the result.
pub struct Once {
    pub child: Arc<dyn BehaviorNode>,
    result: Option<BehaviorStatus>,
}

impl Once {
    pub fn new(child: Arc<dyn BehaviorNode>) -> Self {
        Self {
            child,
            result: None,
        }
    }
}

impl BehaviorNode for Once {
    fn tick(&self, _entity: Entity, _ctx: &mut BehaviorContext) -> BehaviorStatus {
        if let Some(cached) = self.result {
            return cached;
        }
        // Note: This requires mutability, which the trait doesn't provide.
        // In practice, use the MutableOnce wrapper or store state in memory.
        self.child.tick(_entity, _ctx)
    }
}

/// Mutable version of Once that stores result in context memory.
pub struct MutableOnce {
    pub child: Arc<dyn BehaviorNode>,
    pub memory_key: String,
}

impl BehaviorNode for MutableOnce {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if ctx.memory.memories.contains_key(&self.memory_key) {
            if let Some(MemoryEntry::Boolean(true)) = ctx.memory.memories.get(&self.memory_key) {
                return BehaviorStatus::Success;
            }
        }

        let result = self.child.tick(entity, ctx);

        match result {
            BehaviorStatus::Success => {
                ctx.memory
                    .memories
                    .insert(self.memory_key.clone(), MemoryEntry::Boolean(true));
            }
            BehaviorStatus::Failure => {
                ctx.memory
                    .memories
                    .insert(self.memory_key.clone(), MemoryEntry::Boolean(false));
            }
            _ => {}
        }

        result
    }
}

/// Leaf node: Move to a target position.
pub struct MoveToPosition;

impl ActionNode for MoveToPosition {
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if let Some(target_pos) = &ctx.memory.last_target_pos {
            let current = ctx
                .position
                .map(|p| BlockPos::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32));

            if let Some(current_pos) = current {
                if current_pos == *target_pos {
                    return BehaviorStatus::Success;
                }

                // If we have a path, follow it
                if !ctx.memory.path_finished() {
                    return BehaviorStatus::Running;
                }

                // Need to find a path (done externally)
                BehaviorStatus::Running
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Leaf node: Wait for a duration.
pub struct Wait {
    pub ticks: u32,
}

impl ActionNode for Wait {
    fn tick(&self, _entity: Entity, _ctx: &mut BehaviorContext) -> BehaviorStatus {
        // In a real implementation, you'd track elapsed ticks in memory
        BehaviorStatus::Success
    }
}

/// Leaf node: Attack the current target.
pub struct AttackTarget;

impl ActionNode for AttackTarget {
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if ctx.memory.current_target.is_some() {
            // In a real implementation, you'd trigger an attack animation/event
            BehaviorStatus::Success
        } else {
            BehaviorStatus::Failure
        }
    }
}

/// Leaf node: Flee from a threat.
pub struct FleeFromThreat;

impl ActionNode for FleeFromThreat {
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        // Find the highest threat in memory and set a flee target opposite to it
        let mut max_threat = 0.0f32;
        let mut threat_pos = None;

        for (_entity, info) in &ctx.memory.known_positions {
            if info.threat_level > max_threat {
                max_threat = info.threat_level;
                threat_pos = Some(info.position);
            }
        }

        if max_threat > 0.5 {
            if let Some(threat) = threat_pos {
                if let Some(current) = ctx.position {
                    let flee_x = current.x + (current.x - threat.x as f64) * 5.0;
                    let flee_z = current.z + (current.z - threat.z as f64) * 5.0;
                    ctx.memory.last_target_pos = Some(BlockPos::new(
                        flee_x.floor() as i32,
                        current.y.floor() as i32,
                        flee_z.floor() as i32,
                    ));
                }
            }
            BehaviorStatus::Running
        } else {
            BehaviorStatus::Success
        }
    }
}