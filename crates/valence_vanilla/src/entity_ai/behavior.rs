use std::sync::Arc;

use bevy_ecs::prelude::*;
use valence_entity::Position;
use valence_protocol::BlockPos;

use super::{BehaviorContext, BehaviorNode, BehaviorStatus};
use super::memory::{EntityMemory, MemoryEntry};

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
        if let Some(ref root) = self.root {
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
        if let Some(ref target_pos) = ctx.memory.last_target_pos {
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
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        let key = format!("wait_counter_{}", self.ticks);
        let counter = match ctx.memory.memories.get(&key) {
            Some(MemoryEntry::Integer(n)) => *n as u32,
            _ => 0,
        };

        if counter >= self.ticks {
            ctx.memory.memories.remove(&key);
            BehaviorStatus::Success
        } else {
            ctx.memory
                .memories
                .insert(key, MemoryEntry::Integer((counter + 1) as i64));
            BehaviorStatus::Running
        }
    }
}

/// Leaf node: Set a memory value.
pub struct SetMemory {
    pub key: String,
    pub value: MemoryEntry,
}

impl ActionNode for SetMemory {
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        ctx.memory.memories.insert(self.key.clone(), self.value.clone());
        BehaviorStatus::Success
    }
}

/// Leaf node: Check if entity is on ground.
pub struct IsOnGround;

impl ActionNode for IsOnGround {
    fn tick(&self, _entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if let Some(MemoryEntry::Boolean(grounded)) = ctx.memory.memories.get("on_ground") {
            if *grounded {
                BehaviorStatus::Success
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingAction {
        count: Arc<AtomicU32>,
    }

    impl ActionNode for CountingAction {
        fn tick(&self, _entity: Entity, _ctx: &mut BehaviorContext) -> BehaviorStatus {
            self.count.fetch_add(1, Ordering::SeqCst);
            BehaviorStatus::Success
        }
    }

    #[test]
    fn test_sequence_success() {
        let count = Arc::new(AtomicU32::new(0));
        let seq = super::super::Sequence {
            children: vec![
                Arc::new(CountingAction {
                    count: count.clone(),
                }),
                Arc::new(CountingAction {
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
}
