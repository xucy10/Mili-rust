use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_entity::{HitboxShape, Position, Velocity};
use valence_math::{Aabb, DVec3};
use valence_protocol::{BlockPos, BlockState};
use valence_server::layer::chunk::ChunkLayer;

/// Component for a physics body.
#[derive(Component)]
pub struct PhysicsBody {
    /// Current velocity in m/s.
    pub velocity: DVec3,
    /// Acceleration applied each tick in m/s².
    pub acceleration: DVec3,
    /// Mass of the body in kg (affects impulse response).
    pub mass: f32,
    /// Drag coefficient (0.0 = no drag, 1.0 = instant stop).
    pub drag: f32,
    /// Gravity multiplier (1.0 = normal gravity, 0.0 = no gravity).
    pub gravity_multiplier: f32,
    /// Whether the body is on the ground.
    pub on_ground: bool,
    /// Whether to check for block collisions.
    pub collides_with_blocks: bool,
    /// Whether to check for entity collisions.
    pub collides_with_entities: bool,
    /// Maximum fall speed in blocks/tick (vanilla: 100 blocks/tick cap).
    pub max_fall_speed: f64,
    /// Terminal velocity in m/s.
    pub terminal_velocity: f64,
}

impl Default for PhysicsBody {
    fn default() -> Self {
        Self {
            velocity: DVec3::ZERO,
            acceleration: DVec3::ZERO,
            mass: 1.0,
            drag: 0.0,
            gravity_multiplier: 1.0,
            on_ground: false,
            collides_with_blocks: true,
            collides_with_entities: true,
            max_fall_speed: 100.0,
            terminal_velocity: 78.4, // vanilla: ~4.9 * 16
        }
    }
}

impl PhysicsBody {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a static body that doesn't move.
    pub fn static_body() -> Self {
        Self {
            gravity_multiplier: 0.0,
            ..Default::default()
        }
    }

    /// Creates a kinematic body (gravity but no collisions).
    pub fn kinematic() -> Self {
        Self {
            collides_with_blocks: false,
            collides_with_entities: false,
            ..Default::default()
        }
    }

    /// Apply an impulse to the body.
    pub fn apply_impulse(&mut self, impulse: DVec3) {
        self.velocity += impulse / self.mass as f64;
    }

    /// Apply a force over one tick.
    pub fn apply_force(&mut self, force: DVec3) {
        self.acceleration += force / self.mass as f64;
    }

    /// Set velocity directly.
    pub fn set_velocity(&mut self, velocity: DVec3) {
        self.velocity = velocity;
    }

    /// Get the current speed (magnitude of velocity).
    pub fn speed(&self) -> f64 {
        self.velocity.length()
    }

    /// Whether the body is moving downward.
    pub fn is_falling(&self) -> bool {
        self.velocity.y < 0.0
    }

    /// Whether the body is moving upward.
    pub fn is_jumping(&self) -> bool {
        self.velocity.y > 0.0
    }
}

/// Hitbox for an entity (width x height).
#[derive(Component)]
pub struct Hitbox {
    pub width: f32,
    pub height: f32,
}

impl Hitbox {
    pub fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Create a hitbox from an Aabb (using the bottom-center as position reference).
    pub fn from_aabb(aabb: Aabb) -> Self {
        let size = aabb.max() - aabb.min();
        Self {
            width: size.x.max(size.z) as f32,
            height: size.y as f32,
        }
    }

    /// Get the AABB centered at the given position.
    pub fn aabb_at(&self, pos: DVec3) -> Aabb {
        let half_w = self.width as f64 / 2.0;
        Aabb::new(
            DVec3::new(pos.x - half_w, pos.y, pos.z - half_w),
            DVec3::new(pos.x + half_w, pos.y + self.height as f64, pos.z + half_w),
        )
    }
}

/// Result of a collision check.
#[derive(Debug, Clone)]
pub struct CollisionResult {
    /// Whether a collision occurred.
    pub hit: bool,
    /// Adjusted position after collision.
    pub position: DVec3,
    /// Adjusted velocity after collision.
    pub velocity: DVec3,
    /// Whether the entity landed on ground.
    pub on_ground: bool,
    /// The face that was hit (if any).
    pub hit_face: Option<HitFace>,
}

/// Which face of a block was hit during collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitFace {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

impl CollisionResult {
    /// No collision occurred.
    pub fn none(position: DVec3, velocity: DVec3) -> Self {
        Self {
            hit: false,
            position,
            velocity,
            on_ground: false,
            hit_face: None,
        }
    }

    /// A collision occurred.
    pub fn hit(
        position: DVec3,
        velocity: DVec3,
        on_ground: bool,
        hit_face: Option<HitFace>,
    ) -> Self {
        Self {
            hit: true,
            position,
            velocity,
            on_ground,
            hit_face,
        }
    }
}

/// Marker component for falling blocks (sand, gravel, etc.).
#[derive(Component)]
pub struct FallingBlock;

/// Data component for falling blocks, storing the block to place on landing.
#[derive(Component)]
pub struct FallingBlockData {
    pub block_state: BlockState,
    pub drop_item: bool,
}

impl Default for FallingBlockData {
    fn default() -> Self {
        Self {
            block_state: BlockState::default(),
            drop_item: true,
        }
    }
}

/// Marker component for blocks that should fall when unsupported.
#[derive(Component)]
pub struct GravityBlock;

/// Plugin for physics simulation.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<FallingBlockLandEvent>()
            .add_systems(
                Update,
                (
                    physics_system,
                    falling_block_system,
                )
                    .chain(),
            );
    }
}

/// Event fired when a falling block lands.
#[derive(Event)]
pub struct FallingBlockLandEvent {
    pub entity: Entity,
    pub position: BlockPos,
    pub block_state: BlockState,
}

/// Constants for physics simulation.
pub mod consts {
    /// Gravitational acceleration in m/s² (blocks/tick² * 400).
    pub const GRAVITY: f64 = 28.0; // vanilla: 0.08 blocks/tick² * 400 = ~32, but adjusted
    /// Drag in air.
    pub const AIR_DRAG: f64 = 0.02;
    /// Drag in water.
    pub const WATER_DRAG: f64 = 0.02;
    /// Drag in lava.
    pub const LAVA_DRAG: f64 = 0.02;
    /// Terminal velocity in blocks/tick.
    pub const TERMINAL_VELOCITY: f64 = 3.92; // vanilla: 0.098 * 40
    /// Speed of light in blocks/tick (not really, but a sanity cap).
    pub const MAX_VELOCITY: f64 = 100.0;
    /// Height of a player in blocks.
    pub const PLAYER_HEIGHT: f64 = 1.8;
    /// Width of a player in blocks.
    pub const PLAYER_WIDTH: f64 = 0.6;
    /// Step height for climbing blocks (like stairs).
    pub const STEP_HEIGHT: f64 = 0.6;
}

/// The main physics system. Applies gravity, drag, and block collisions.
pub fn physics_system(
    mut query: Query<(
        Entity,
        &mut PhysicsBody,
        &mut Position,
        &mut Velocity,
        &HitboxShape,
    )>,
    chunk_layers: Query<&ChunkLayer>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds_f64().min(0.1); // Cap at 100ms to prevent tunneling

    for (entity, mut body, mut pos, mut velocity, hitbox_shape) in &mut query {
        if !body.collides_with_blocks {
            // Simple physics without collision
            apply_gravity(&mut body, dt);
            apply_drag(&mut body, dt);
            apply_acceleration(&mut body, dt);

            // Clamp velocity
            clamp_velocity(&mut body);

            // Update position
            pos.0 += body.velocity * dt;
            velocity.0 = body.velocity.as_(); // Sync to protocol velocity
            continue;
        }

        // Apply gravity
        apply_gravity(&mut body, dt);

        // Apply drag
        apply_drag(&mut body, dt);

        // Apply acceleration
        apply_acceleration(&mut body, dt);

        // Clamp velocity to prevent tunneling
        clamp_velocity(&mut body);

        // Calculate intended new position
        let intended_pos = pos.0 + body.velocity * dt;

        // Check block collisions
        let aabb = hitbox_shape.get();

        if let Ok(chunk_layer) = chunk_layers.get_single() {
            let collision = check_block_collision(chunk_layer, pos.0, intended_pos, aabb);

            pos.0 = collision.position;
            body.velocity = collision.velocity;
            body.on_ground = collision.on_ground;
            velocity.0 = body.velocity.as_(); // Sync to protocol velocity
        } else {
            pos.0 = intended_pos;
            velocity.0 = body.velocity.as_();
        }
    }
}

/// Apply gravity to the physics body.
fn apply_gravity(body: &mut PhysicsBody, dt: f64) {
    if body.gravity_multiplier == 0.0 {
        return;
    }

    // Vanilla gravity: 0.08 blocks/tick² = ~28 blocks/s²
    let gravity = consts::GRAVITY * body.gravity_multiplier as f64;
    body.velocity.y -= gravity * dt;

    // Terminal velocity
    if body.velocity.y < -body.terminal_velocity {
        body.velocity.y = -body.terminal_velocity;
    }
}

/// Apply drag to the physics body.
fn apply_drag(body: &mut PhysicsBody, dt: f64) {
    let drag = body.drag as f64;
    if drag > 0.0 {
        body.velocity *= 1.0 - drag * dt;
    } else {
        // Default air drag (very small)
        body.velocity *= 1.0 - consts::AIR_DRAG * dt;
    }
}

/// Apply acceleration to the physics body.
fn apply_acceleration(body: &mut PhysicsBody, dt: f64) {
    body.velocity += body.acceleration * dt;
    // Reset acceleration for next frame (it's applied per-tick, not continuous)
    body.acceleration = DVec3::ZERO;
}

/// Clamp velocity to prevent tunneling and infinite speeds.
fn clamp_velocity(body: &mut PhysicsBody) {
    let max = consts::MAX_VELOCITY;
    body.velocity.x = body.velocity.x.clamp(-max, max);
    body.velocity.y = body.velocity.y.clamp(-body.max_fall_speed, max);
    body.velocity.z = body.velocity.z.clamp(-max, max);
}

/// Check block collisions and return adjusted position/velocity.
pub fn check_block_collision(
    chunk_layer: &ChunkLayer,
    current_pos: DVec3,
    intended_pos: DVec3,
    entity_aabb: Aabb,
) -> CollisionResult {
    let velocity = intended_pos - current_pos;

    // Early out if not moving
    if velocity.x.abs() < 1e-10 && velocity.y.abs() < 1e-10 && velocity.z.abs() < 1e-10 {
        return CollisionResult::none(current_pos, DVec3::ZERO);
    }

    // Expand the AABB to the sweep area
    let sweep_min = current_pos.min(intended_pos) + entity_aabb.min();
    let sweep_max = current_pos.max(intended_pos) + entity_aabb.max();

    let min_block = BlockPos::new(
        sweep_min.x.floor() as i32 - 1,
        sweep_min.y.floor() as i32 - 1,
        sweep_min.z.floor() as i32 - 1,
    );
    let max_block = BlockPos::new(
        sweep_max.x.ceil() as i32 + 1,
        sweep_max.y.ceil() as i32 + 1,
        sweep_max.z.ceil() as i32 + 1,
    );

    // Collect all solid block AABBs in the sweep area
    let mut block_aabbs: Vec<(Aabb, BlockPos)> = Vec::new();

    for bx in min_block.x..=max_block.x {
        for by in min_block.y..=max_block.y {
            for bz in min_block.z..=max_block.z {
                let block_pos = BlockPos::new(bx, by, bz);
                if let Some(block_ref) = chunk_layer.block(block_pos) {
                    let state = block_ref.state;
                    // Check if the block has collision
                    if state.blocks_motion() {
                        // Simple full-block AABB for solid blocks
                        let block_aabb = Aabb::new(
                            DVec3::new(bx as f64, by as f64, bz as f64),
                            DVec3::new(bx as f64 + 1.0, by as f64 + 1.0, bz as f64 + 1.0),
                        );
                        // Also check detailed collision shapes
                        for shape in state.collision_shapes() {
                            let shaped_aabb = Aabb::new(
                                DVec3::new(
                                    bx as f64 + shape.min().x,
                                    by as f64 + shape.min().y,
                                    bz as f64 + shape.min().z,
                                ),
                                DVec3::new(
                                    bx as f64 + shape.max().x,
                                    by as f64 + shape.max().y,
                                    bz as f64 + shape.max().z,
                                ),
                            );
                            block_aabbs.push((shaped_aabb, block_pos));
                        }
                    }
                }
            }
        }
    }

    // Sweep the entity AABB along the velocity vector
    // We do axis-separated collision resolution (like vanilla Minecraft)
    let mut result_pos = current_pos;
    let mut result_vel = DVec3::new(velocity.x, velocity.y, velocity.z);
    let mut on_ground = false;
    let mut hit_face = None;

    // Resolve Y axis first (gravity is most important)
    let new_y = current_pos.y + velocity.y;
    let entity_aabb_at_new_y = Aabb::new(
        DVec3::new(
            current_pos.x + entity_aabb.min().x,
            new_y + entity_aabb.min().y,
            current_pos.z + entity_aabb.min().z,
        ),
        DVec3::new(
            current_pos.x + entity_aabb.max().x,
            new_y + entity_aabb.max().y,
            current_pos.z + entity_aabb.max().z,
        ),
    );

    let mut collided_y = false;
    for (block_aabb, block_pos) in &block_aabbs {
        if entity_aabb_at_new_y.intersects(*block_aabb) {
            if velocity.y < 0.0 {
                // Landing on top of block
                result_pos.y = block_aabb.max().y - entity_aabb.min().y;
                result_vel.y = 0.0;
                on_ground = true;
                hit_face = Some(HitFace::Top);
            } else {
                // Hitting bottom of block
                result_pos.y = block_aabb.min().y - entity_aabb.max().y;
                result_vel.y = 0.0;
                hit_face = Some(HitFace::Bottom);
            }
            collided_y = true;
            break;
        }
    }

    if !collided_y {
        result_pos.y = new_y;
    }

    // Resolve X axis
    let new_x = result_pos.x + velocity.x;
    let entity_aabb_at_new_x = Aabb::new(
        DVec3::new(
            new_x + entity_aabb.min().x,
            result_pos.y + entity_aabb.min().y,
            result_pos.z + entity_aabb.min().z,
        ),
        DVec3::new(
            new_x + entity_aabb.max().x,
            result_pos.y + entity_aabb.max().y,
            result_pos.z + entity_aabb.max().z,
        ),
    );

    let mut collided_x = false;
    for (block_aabb, block_pos) in &block_aabbs {
        if entity_aabb_at_new_x.intersects(*block_aabb) {
            if velocity.x > 0.0 {
                result_pos.x = block_aabb.min().x - entity_aabb.max().x;
                hit_face = Some(HitFace::West);
            } else {
                result_pos.x = block_aabb.max().x - entity_aabb.min().x;
                hit_face = Some(HitFace::East);
            }
            result_vel.x = 0.0;
            collided_x = true;
            break;
        }
    }

    if !collided_x {
        result_pos.x = new_x;
    }

    // Resolve Z axis
    let new_z = result_pos.z + velocity.z;
    let entity_aabb_at_new_z = Aabb::new(
        DVec3::new(
            result_pos.x + entity_aabb.min().x,
            result_pos.y + entity_aabb.min().y,
            new_z + entity_aabb.min().z,
        ),
        DVec3::new(
            result_pos.x + entity_aabb.max().x,
            result_pos.y + entity_aabb.max().y,
            new_z + entity_aabb.max().z,
        ),
    );

    let mut collided_z = false;
    for (block_aabb, block_pos) in &block_aabbs {
        if entity_aabb_at_new_z.intersects(*block_aabb) {
            if velocity.z > 0.0 {
                result_pos.z = block_aabb.min().z - entity_aabb.max().z;
                hit_face = Some(HitFace::North);
            } else {
                result_pos.z = block_aabb.max().z - entity_aabb.min().z;
                hit_face = Some(HitFace::South);
            }
            result_vel.z = 0.0;
            collided_z = true;
            break;
        }
    }

    if !collided_z {
        result_pos.z = new_z;
    }

    let any_hit = collided_x || collided_y || collided_z;

    CollisionResult::hit(result_pos, result_vel, on_ground, hit_face)
}

/// System that handles falling blocks (sand, gravel, etc.).
pub fn falling_block_system(
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut PhysicsBody,
        &mut Position,
        &FallingBlockData,
    )>,
    chunk_layers: Query<&ChunkLayer>,
    mut land_events: EventWriter<FallingBlockLandEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds_f64();

    for (entity, mut body, mut pos, block_data) in &mut query {
        // Apply gravity
        body.velocity.y -= consts::GRAVITY * dt;

        // Terminal velocity for falling blocks
        if body.velocity.y < -consts::TERMINAL_VELOCITY {
            body.velocity.y = -consts::TERMINAL_VELOCITY;
        }

        let intended_pos = pos.0 + body.velocity * dt;

        // Check collision with blocks
        let entity_aabb = Aabb::new(
            DVec3::new(pos.0.x - 0.49, pos.0.y, pos.0.z - 0.49),
            DVec3::new(pos.0.x + 0.49, pos.0.y + 0.98, pos.0.z + 0.49),
        );

        if let Ok(chunk_layer) = chunk_layers.get_single() {
            let collision = check_block_collision(chunk_layer, pos.0, intended_pos, entity_aabb);

            if collision.hit {
                // Place the block where the falling block landed
                let place_pos = BlockPos::new(
                    collision.position.x.floor() as i32,
                    collision.position.y.floor() as i32,
                    collision.position.z.floor() as i32,
                );

                land_events.write(FallingBlockLandEvent {
                    entity,
                    position: place_pos,
                    block_state: block_data.block_state,
                });

                // Despawn the falling block entity
                commands.entity(entity).despawn();
            } else {
                pos.0 = collision.position;
                body.velocity = collision.velocity;
            }
        } else {
            pos.0 = intended_pos;
        }
    }
}

/// Utility function to spawn a falling block entity.
pub fn spawn_falling_block(
    commands: &mut Commands,
    block_state: BlockState,
    position: DVec3,
) -> Entity {
    commands
        .spawn((
            FallingBlock,
            FallingBlockData {
                block_state,
                drop_item: true,
            },
            PhysicsBody {
                gravity_multiplier: 1.0,
                collides_with_blocks: true,
                collides_with_entities: false,
                ..Default::default()
            },
            Position::new(position),
            Velocity::default(),
            HitboxShape(Aabb::new(
                DVec3::new(-0.49, 0.0, -0.49),
                DVec3::new(0.49, 0.98, 0.49),
            )),
        ))
        .id()
}

/// Check if a block should fall (sand, gravel, etc.).
pub fn should_block_fall(chunk_layer: &ChunkLayer, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    if let Some(below_block) = chunk_layer.block(below) {
        // Block falls if the block below is air or replaceable
        below_block.state.is_air() || below_block.state.is_replaceable()
    } else {
        // Below the world = falls
        true
    }
}

/// Check if a block at a position can be replaced by a falling block.
pub fn can_place_falling_block(chunk_layer: &ChunkLayer, pos: BlockPos) -> bool {
    if let Some(block) = chunk_layer.block(pos) {
        block.state.is_air() || block.state.is_replaceable()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hitbox_aabb() {
        let hitbox = Hitbox::new(0.6, 1.8);
        let aabb = hitbox.aabb_at(DVec3::new(0.0, 0.0, 0.0));
        assert_eq!(aabb.min(), DVec3::new(-0.3, 0.0, -0.3));
        assert_eq!(aabb.max(), DVec3::new(0.3, 1.8, 0.3));
    }

    #[test]
    fn test_physics_body_defaults() {
        let body = PhysicsBody::default();
        assert_eq!(body.mass, 1.0);
        assert_eq!(body.gravity_multiplier, 1.0);
        assert!(!body.on_ground);
    }

    #[test]
    fn test_collision_result_none() {
        let result = CollisionResult::none(DVec3::ZERO, DVec3::ZERO);
        assert!(!result.hit);
        assert_eq!(result.position, DVec3::ZERO);
    }
}
