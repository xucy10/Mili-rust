use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use rand::Rng;
use valence_entity::Position;
use valence_math::DVec3;
use valence_server::client::Client;

use crate::combat::{AttackCooldown, CombatStats, Health};
use crate::entity_ai::memory::EntityMemory;
use crate::entity_ai::perception::Perception;
use crate::physics::PhysicsBody;
use crate::villager::{VillagerAi, VillagerProfession};

pub struct MobSpawningPlugin;

impl Plugin for MobSpawningPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MobSpawnSettings>()
            .add_systems(Update, (mob_spawning_system, despawn_distant_mobs));
    }
}

#[derive(Resource)]
pub struct MobSpawnSettings {
    pub hostile_spawn_rate: f32,
    pub passive_spawn_rate: f32,
    pub max_hostile_per_player: u32,
    pub max_passive_per_player: u32,
    pub spawn_range: i32,
    pub despawn_range: i32,
}

impl Default for MobSpawnSettings {
    fn default() -> Self {
        Self {
            hostile_spawn_rate: 0.1,
            passive_spawn_rate: 0.02,
            max_hostile_per_player: 70,
            max_passive_per_player: 10,
            spawn_range: 48,
            despawn_range: 128,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MobType {
    Zombie,
    Skeleton,
    Creeper,
    Spider,
    Enderman,
    Villager,
    Pig,
    Cow,
    Sheep,
    Chicken,
    Blaze,
    Ghast,
    WitherSkeleton,
    Endermite,
}

impl MobType {
    pub fn is_hostile(&self) -> bool {
        matches!(
            self,
            MobType::Zombie
                | MobType::Skeleton
                | MobType::Creeper
                | MobType::Spider
                | MobType::Enderman
                | MobType::Blaze
                | MobType::Ghast
                | MobType::WitherSkeleton
                | MobType::Endermite
        )
    }

    pub fn health(&self) -> f32 {
        match self {
            MobType::Zombie => 20.0,
            MobType::Skeleton => 20.0,
            MobType::Creeper => 20.0,
            MobType::Spider => 16.0,
            MobType::Enderman => 40.0,
            MobType::Villager => 20.0,
            MobType::Pig => 10.0,
            MobType::Cow => 10.0,
            MobType::Sheep => 8.0,
            MobType::Chicken => 4.0,
            MobType::Blaze => 20.0,
            MobType::Ghast => 10.0,
            MobType::WitherSkeleton => 20.0,
            MobType::Endermite => 8.0,
        }
    }

    pub fn combat_stats(&self) -> CombatStats {
        match self {
            MobType::Zombie => CombatStats::zombie(),
            MobType::Skeleton => CombatStats::skeleton(),
            MobType::Creeper => CombatStats::creeper(),
            MobType::Spider => CombatStats::spider(),
            MobType::Enderman => CombatStats::enderman(),
            MobType::Villager => CombatStats::default(),
            MobType::Pig => CombatStats::default(),
            MobType::Cow => CombatStats::default(),
            MobType::Sheep => CombatStats::default(),
            MobType::Chicken => CombatStats::default(),
            MobType::Blaze => CombatStats {
                attack_damage: 6.0,
                ..Default::default()
            },
            MobType::Ghast => CombatStats {
                attack_damage: 0.0,
                ..Default::default()
            },
            MobType::WitherSkeleton => CombatStats {
                attack_damage: 5.0,
                armor: 2.0,
                ..Default::default()
            },
            MobType::Endermite => CombatStats {
                attack_damage: 2.0,
                ..Default::default()
            },
        }
    }

    pub fn perception(&self) -> Perception {
        match self {
            MobType::Zombie => Perception::with_ranges(40.0, 16.0),
            MobType::Skeleton => Perception::with_ranges(16.0, 16.0),
            MobType::Creeper => Perception::with_ranges(16.0, 16.0),
            MobType::Spider => Perception::with_ranges(16.0, 16.0),
            MobType::Enderman => Perception::with_ranges(64.0, 16.0),
            _ => Perception::with_ranges(16.0, 8.0),
        }
    }

    pub fn overworld_hostile_mobs() -> Vec<MobType> {
        vec![
            MobType::Zombie,
            MobType::Skeleton,
            MobType::Creeper,
            MobType::Spider,
            MobType::Enderman,
        ]
    }

    pub fn overworld_passive_mobs() -> Vec<MobType> {
        vec![
            MobType::Pig,
            MobType::Cow,
            MobType::Sheep,
            MobType::Chicken,
            MobType::Villager,
        ]
    }

    pub fn nether_hostile_mobs() -> Vec<MobType> {
        vec![
            MobType::Zombie,
            MobType::Skeleton,
            MobType::Blaze,
            MobType::Ghast,
            MobType::WitherSkeleton,
            MobType::Endermite,
        ]
    }

    pub fn end_hostile_mobs() -> Vec<MobType> {
        vec![MobType::Enderman, MobType::Endermite]
    }
}

#[derive(Component)]
pub struct Mob;

#[derive(Component)]
pub struct HostileMob;

#[derive(Component)]
pub struct PassiveMob;

fn mob_spawning_system(
    mut commands: Commands,
    settings: Res<MobSpawnSettings>,
    player_positions: Query<&Position, With<Client>>,
    mob_count: Query<(), With<Mob>>,
) {
    let mut rng = rand::thread_rng();

    let player_count = player_positions.iter().len().max(1);
    let mob_count_val = mob_count.iter().count() as u32;
    if mob_count_val >= settings.max_hostile_per_player * player_count as u32 {
        return;
    }

    for pos in &player_positions {
        if rng.gen::<f32>() < settings.hostile_spawn_rate {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist = rng.gen_range(24.0..settings.spawn_range as f32);
            let spawn_x = pos.0.x + angle.cos() as f64 * dist as f64;
            let spawn_z = pos.0.z + angle.sin() as f64 * dist as f64;
            let spawn_y = pos.0.y + rng.gen_range(-5.0..10.0);

            let mob_types = MobType::overworld_hostile_mobs();
            let mob_type = mob_types[rng.gen_range(0..mob_types.len())];

            spawn_mob(
                &mut commands,
                mob_type,
                DVec3::new(spawn_x, spawn_y, spawn_z),
            );
        }

        if rng.gen::<f32>() < settings.passive_spawn_rate {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let dist = rng.gen_range(10.0..40.0);
            let spawn_x = pos.0.x + angle.cos() as f64 * dist as f64;
            let spawn_z = pos.0.z + angle.sin() as f64 * dist as f64;
            let spawn_y = pos.0.y;

            let mob_types = MobType::overworld_passive_mobs();
            let mob_type = mob_types[rng.gen_range(0..mob_types.len())];

            spawn_mob(
                &mut commands,
                mob_type,
                DVec3::new(spawn_x, spawn_y, spawn_z),
            );
        }
    }
}

pub fn spawn_mob(commands: &mut Commands, mob_type: MobType, position: DVec3) -> Entity {
    let health = Health::new(mob_type.health());
    let combat_stats = mob_type.combat_stats();
    let perception = mob_type.perception();

    let entity = commands
        .spawn((
            Mob,
            Position(position),
            health,
            combat_stats,
            AttackCooldown::default(),
            perception,
            EntityMemory::new(),
            PhysicsBody::new(),
        ))
        .id();

    if mob_type.is_hostile() {
        commands.entity(entity).insert(HostileMob);
    } else {
        commands.entity(entity).insert(PassiveMob);
    }

    if mob_type == MobType::Villager {
        let professions = [
            VillagerProfession::Farmer,
            VillagerProfession::Librarian,
            VillagerProfession::Cleric,
            VillagerProfession::Armorer,
            VillagerProfession::Weaponsmith,
            VillagerProfession::Toolsmith,
            VillagerProfession::Butcher,
            VillagerProfession::Leatherworker,
            VillagerProfession::Mason,
            VillagerProfession::Fisherman,
            VillagerProfession::Fletcher,
            VillagerProfession::Shepherd,
            VillagerProfession::Cartographer,
        ];
        let profession = professions[rand::thread_rng().gen_range(0..professions.len())];
        commands.entity(entity).insert(VillagerAi::new(profession));
    }

    entity
}

fn despawn_distant_mobs(
    mut commands: Commands,
    settings: Res<MobSpawnSettings>,
    mob_query: Query<(Entity, &Position), With<Mob>>,
    player_positions: Query<&Position, With<Client>>,
) {
    let despawn_range_sq = (settings.despawn_range as f64).powi(2);

    for (entity, mob_pos) in &mob_query {
        let mut nearest_player_dist = f64::MAX;

        for player_pos in &player_positions {
            let dist = (mob_pos.0 - player_pos.0).length_squared();
            if dist < nearest_player_dist {
                nearest_player_dist = dist;
            }
        }

        if nearest_player_dist > despawn_range_sq {
            commands.entity(entity).despawn();
        }
    }
}