use std::sync::Arc;

use bevy_ecs::prelude::*;

use crate::entity_ai::{BehaviorContext, BehaviorNode, BehaviorStatus};

#[derive(Component)]
pub struct BehaviorTree {
    root: Arc<dyn BehaviorNode>,
}

impl BehaviorTree {
    pub fn new(root: Arc<dyn BehaviorNode>) -> Self {
        Self { root }
    }

    pub fn tick(&self, entity: bevy_ecs::entity::Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        self.root.tick(entity, ctx)
    }
}

pub struct BehaviorTreeBuilder {
    root: Option<Arc<dyn BehaviorNode>>,
}

impl BehaviorTreeBuilder {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn with_root(mut self, node: Arc<dyn BehaviorNode>) -> Self {
        self.root = Some(node);
        self
    }

    pub fn build(self) -> BehaviorTree {
        BehaviorTree::new(self.root.unwrap_or_else(|| Arc::new(crate::entity_ai::Sequence {
            children: vec![],
        })))
    }
}