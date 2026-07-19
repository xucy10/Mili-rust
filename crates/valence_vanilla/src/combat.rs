use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_entity::Position;
use valence_math::DVec3;

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<EntityDamageEvent>()
            .add_event::<EntityDeathEvent>()
            .add_event::<EntityAttackEvent>()
            .add_systems(
                Update,
                (
                    process_attacks,
                    process_damage,
                    process_death,
                    attack_cooldown_system,
                ),
            );
    }
}

#[derive(Component, Debug, Clone)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 20.0,
            max: 20.0,
        }
    }
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.max);
    }

    pub fn set_full(&mut self) {
        self.current = self.max;
    }
}

#[derive(Component, Debug, Clone)]
pub struct AttackCooldown {
    pub remaining_ticks: u32,
    pub max_ticks: u32,
}

impl Default for AttackCooldown {
    fn default() -> Self {
        Self {
            remaining_ticks: 0,
            max_ticks: 10,
        }
    }
}

impl AttackCooldown {
    pub fn is_ready(&self) -> bool {
        self.remaining_ticks == 0
    }

    pub fn reset(&mut self) {
        self.remaining_ticks = self.max_ticks;
    }

    pub fn attack_strength(&self) -> f32 {
        if self.max_ticks == 0 {
            return 1.0;
        }
        1.0 - (self.remaining_ticks as f32 / self.max_ticks as f32)
    }
}

fn attack_cooldown_system(mut query: Query<&mut AttackCooldown>) {
    for mut cooldown in &mut query {
        if cooldown.remaining_ticks > 0 {
            cooldown.remaining_ticks -= 1;
        }
    }
}

#[derive(Component, Debug, Clone, Default)]
pub struct CombatStats {
    pub attack_damage: f32,
    pub armor: f32,
    pub armor_toughness: f32,
    pub knockback_resistance: f32,
    pub attack_speed: f32,
}

impl CombatStats {
    pub fn player_defaults() -> Self {
        Self {
            attack_damage: 1.0,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            attack_speed: 4.0,
        }
    }

    pub fn zombie() -> Self {
        Self {
            attack_damage: 3.0,
            armor: 2.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            attack_speed: 1.0,
        }
    }

    pub fn skeleton() -> Self {
        Self {
            attack_damage: 2.0,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            attack_speed: 1.0,
        }
    }

    pub fn creeper() -> Self {
        Self {
            attack_damage: 0.0,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            attack_speed: 1.0,
        }
    }

    pub fn spider() -> Self {
        Self {
            attack_damage: 2.0,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 0.0,
            attack_speed: 1.0,
        }
    }

    pub fn enderman() -> Self {
        Self {
            attack_damage: 7.0,
            armor: 0.0,
            armor_toughness: 0.0,
            knockback_resistance: 1.0,
            attack_speed: 1.0,
        }
    }
}

#[derive(Event, Debug, Clone)]
pub struct EntityDamageEvent {
    pub entity: Entity,
    pub damage: f32,
    pub source: DamageSource,
    pub attacker: Option<Entity>,
}

#[derive(Clone, Debug)]
pub enum DamageSource {
    Melee,
    Projectile,
    Fall(f32),
    Fire,
    Lava,
    Drown,
    Void,
    Explosion(f32),
    Magic,
    Starve,
    Cactus,
    Anvil,
    Thorns,
}

#[derive(Event, Debug, Clone)]
pub struct EntityDeathEvent {
    pub entity: Entity,
    pub position: DVec3,
    pub damage_source: DamageSource,
    pub killer: Option<Entity>,
}

#[derive(Event, Debug, Clone)]
pub struct EntityAttackEvent {
    pub attacker: Entity,
    pub target: Entity,
    pub damage: f32,
}

fn process_attacks(
    mut attack_events: EventReader<EntityAttackEvent>,
    mut damage_events: EventWriter<EntityDamageEvent>,
    mut attacker_query: Query<(&Position, &mut AttackCooldown, &CombatStats)>,
) {
    for event in attack_events.read() {
        if let Ok((_, mut cooldown, stats)) = attacker_query.get_mut(event.attacker) {
            if !cooldown.is_ready() {
                continue;
            }

            let strength = cooldown.attack_strength();
            let damage = event.damage * strength;

            cooldown.max_ticks = (20.0 / stats.attack_speed) as u32;
            cooldown.reset();

            damage_events.send(EntityDamageEvent {
                entity: event.target,
                damage,
                source: DamageSource::Melee,
                attacker: Some(event.attacker),
            });
        }
    }
}

fn process_damage(
    mut damage_events: EventReader<EntityDamageEvent>,
    mut death_events: EventWriter<EntityDeathEvent>,
    mut health_query: Query<(&mut Health, &CombatStats, &Position)>,
) {
    for event in damage_events.read() {
        if let Ok((mut health, stats, pos)) = health_query.get_mut(event.entity) {
            let final_damage =
                calculate_final_damage(event.damage, stats.armor, stats.armor_toughness);
            health.damage(final_damage);

            if health.is_dead() {
                death_events.send(EntityDeathEvent {
                    entity: event.entity,
                    position: pos.0,
                    damage_source: event.source.clone(),
                    killer: event.attacker,
                });
            }
        }
    }
}

fn calculate_final_damage(damage: f32, armor: f32, toughness: f32) -> f32 {
    if armor <= 0.0 {
        return damage;
    }

    let armor_points = armor;
    let damage_after = damage * (1.0 - armor_points / (armor_points + 5.0 + toughness / 4.0) / 3.0);
    damage_after.max(0.0)
}

fn process_death(
    mut death_events: EventReader<EntityDeathEvent>,
    mut commands: Commands,
    health_query: Query<&Health>,
) {
    for event in death_events.read() {
        if let Ok(health) = health_query.get(event.entity) {
            if health.is_dead() {
                commands.entity(event.entity).despawn();
            }
        }
    }
}

pub fn calculate_knockback(
    attacker_pos: DVec3,
    target_pos: DVec3,
    strength: f32,
    knockback_resistance: f32,
) -> DVec3 {
    let diff = target_pos - attacker_pos;
    let horizontal_dist = diff.x.hypot(diff.z);

    if horizontal_dist > 0.0 {
        let factor = strength * (1.0 - knockback_resistance);
        let nx = diff.x / horizontal_dist;
        let nz = diff.z / horizontal_dist;

        DVec3::new(
            -nx * factor as f64 + (rand::random::<f64>() - 0.5) * 0.2,
            0.3 * factor as f64,
            -nz * factor as f64 + (rand::random::<f64>() - 0.5) * 0.2,
        )
    } else {
        DVec3::new(0.0, 0.3, 0.0)
    }
}