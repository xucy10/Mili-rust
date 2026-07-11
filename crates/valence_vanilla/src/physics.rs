use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_entity::hitbox::HitboxShape;
use valence_entity::{Position, Velocity};
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
    /// Creates a new physics body with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the velocity directly.
    pub fn set_velocity(&mut self, velocity: DVec3) {
        self.velocity = velocity;
    }

    /// Apply an impulse (instantaneous change in velocity).
    pub fn apply_impulse(&mut self, impulse: DVec3) {
        self.velocity += impulse / self.mass as f64;
    }

    /// Apply continuous acceleration.
    pub fn apply_acceleration(&mut self, acceleration: DVec3) {
        self.acceleration += acceleration;
    }
}

/// Plugin that adds physics simulation systems.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (apply_gravity, apply_drag, integrate_motion, solve_collisions).chain());
    }
}

/// Apply gravity to all physics bodies.
fn apply_gravity(mut query: Query<(&mut PhysicsBody, &Velocity)>, time: Res<Time>) {
    let dt = time.delta_seconds() as f64;
    let gravity = DVec3::new(0.0, -9.81, 0.0);

    for (mut body, _velocity) in &mut query {
        if body.gravity_multiplier > 0.0 {
            body.acceleration += gravity * body.gravity_multiplier as f64;
        }
    }
}

/// Apply drag (air resistance) to all physics bodies.
fn apply_drag(mut query: Query<&mut PhysicsBody>, time: Res<Time>) {
    let dt = time.delta_seconds() as f64;

    for mut body in &mut query {
        if body.drag > 0.0 {
            body.velocity *= (1.0 - body.drag as f64).powf(dt);
        }
    }
}

/// Integrate acceleration into velocity and velocity into position.
fn integrate_motion(
    mut query: Query<(&mut PhysicsBody, &mut Position, &mut Velocity)>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds() as f64;

    for (mut body, mut position, mut velocity) in &mut query {
        // v = v0 + a * dt
        body.velocity += body.acceleration * dt;

        // Clamp to terminal velocity
        if body.velocity.length() > body.terminal_velocity {
            body.velocity = body.velocity.normalize() * body.terminal_velocity;
        }

        // Clamp fall speed
        if body.velocity.y < -body.max_fall_speed {
            body.velocity.y = -body.max_fall_speed;
        }

        // pos = pos0 + v * dt
        position.0 += body.velocity * dt;

        // Update the Velocity component to match
        velocity.0 = body.velocity;

        // Reset acceleration for next frame
        body.acceleration = DVec3::ZERO;
    }
}

/// Solve collisions between physics bodies and the world.
fn solve_collisions(
    mut query: Query<(Entity, &mut PhysicsBody, &mut Position, &HitboxShape)>,
    chunk_layers: Query<&ChunkLayer>,
) {
    for (entity, mut body, mut position, hitbox) in &mut query {
        if !body.collides_with_blocks {
            continue;
        }

        let aabb = hitbox.0.translated(position.0);

        // Check collision with blocks in the chunk layer
        if let Ok(chunk_layer) = chunk_layers.get_single() {
            let min_block = BlockPos::new(
                aabb.min.x.floor() as i32,
                aabb.min.y.floor() as i32,
                aabb.min.z.floor() as i32,
            );
            let max_block = BlockPos::new(
                aabb.max.x.floor() as i32,
                aabb.max.y.floor() as i32,
                aabb.max.z.floor() as i32,
            );

            let mut on_ground = false;

            for x in min_block.x..=max_block.x {
                for y in min_block.y..=max_block.y {
                    for z in min_block.z..=max_block.z {
                        let block_pos = BlockPos::new(x, y, z);
                        if let Some(block_ref) = chunk_layer.block(block_pos) {
                            if block_ref.state.is_solid() {
                                let block_aabb = Aabb {
                                    min: DVec3::new(x as f64, y as f64, z as f64),
                                    max: DVec3::new(x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0),
                                };

                                if let Some(collision) = aabb.intersect(block_aabb) {
                                    // Simple resolution: push out of collision
                                    let penetration = collision.max - collision.min;
                                    let mut min_penetration = f64::MAX;
                                    let mut axis = DVec3::ZERO;

                                    if penetration.x < min_penetration {
                                        min_penetration = penetration.x;
                                        axis = DVec3::new(1.0, 0.0, 0.0);
                                    }
                                    if penetration.y < min_penetration {
                                        min_penetration = penetration.y;
                                        axis = DVec3::new(0.0, 1.0, 0.0);
                                    }
                                    if penetration.z < min_penetration {
                                        min_penetration = penetration.z;
                                        axis = DVec3::new(0.0, 0.0, 1.0);
                                    }

                                    // Determine direction based on center positions
                                    let body_center = aabb.center();
                                    let block_center = block_aabb.center();
                                    let direction = body_center - block_center;

                                    // Push in the direction of least penetration
                                    let push = axis * min_penetration * direction.signum();
                                    position.0 += push;

                                    // Zero velocity in the collision axis
                                    if push.y > 0.0 {
                                        body.velocity.y = body.velocity.y.max(0.0);
                                        on_ground = true;
                                    } else if push.y < 0.0 {
                                        body.velocity.y = body.velocity.y.min(0.0);
                                    }
                                    if push.x.abs() > 0.0 {
                                        body.velocity.x = 0.0;
                                    }
                                    if push.z.abs() > 0.0 {
                                        body.velocity.z = 0.0;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            body.on_ground = on_ground;
        }
    }
}