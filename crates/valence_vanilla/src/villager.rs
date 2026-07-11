use std::collections::HashMap;
use std::time::Duration;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use valence_math::{DVec3, Vec3};
use valence_protocol::{BlockPos, ItemStack};
use valence_server::entity::Position;

/// Villager profession types.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum VillagerProfession {
    None,
    Armorer,
    Butcher,
    Cartographer,
    Cleric,
    Farmer,
    Fisherman,
    Fletcher,
    Leatherworker,
    Librarian,
    Mason,
    #[default]
    Nitwit,
    Shepherd,
    Toolsmith,
    Weaponsmith,
}

impl VillagerProfession {
    pub fn workstation(&self) -> &'static str {
        match self {
            Self::None | Self::Nitwit => "",
            Self::Armorer => "blasting_furnace",
            Self::Butcher => "smoker",
            Self::Cartographer => "cartography_table",
            Self::Cleric => "brewing_stand",
            Self::Farmer => "composter",
            Self::Fisherman => "barrel",
            Self::Fletcher => "fletching_table",
            Self::Leatherworker => "cauldron",
            Self::Librarian => "lectern",
            Self::Mason => "stonecutter",
            Self::Shepherd => "loom",
            Self::Toolsmith => "smithing_table",
            Self::Weaponsmith => "grindstone",
        }
    }

    pub fn max_level(&self) -> i32 {
        match self {
            Self::None | Self::Nitwit => 1,
            _ => 5,
        }
    }

    pub fn can_trade(&self) -> bool {
        !matches!(self, Self::None | Self::Nitwit)
    }
}

/// Villager activity states.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum VillagerActivity {
    Idle,
    Working,
    Resting,
    Gossiping,
    Trading,
    Fleeing,
    Gathering,
    Meeting,
    #[default]
    IdleDefault,
}

impl VillagerActivity {
    pub fn duration(&self) -> Duration {
        match self {
            Self::Idle | Self::IdleDefault => Duration::from_secs(300),
            Self::Working => Duration::from_secs(1200),
            Self::Resting => Duration::from_secs(600),
            Self::Gossiping => Duration::from_secs(300),
            Self::Trading => Duration::from_secs(600),
            Self::Fleeing => Duration::from_secs(30),
            Self::Gathering => Duration::from_secs(300),
            Self::Meeting => Duration::from_secs(600),
        }
    }
}

/// Component for villager AI behavior.
#[derive(Component)]
pub struct VillagerAi {
    pub profession: VillagerProfession,
    pub level: u8,
    pub home_pos: Option<BlockPos>,
    pub work_pos: Option<BlockPos>,
    pub bed_pos: Option<BlockPos>,
    pub current_activity: VillagerActivity,
    pub activity_timer: Duration,
    pub target_pos: Option<DVec3>,
    pub gossip_timer: Duration,
    pub nearby_villagers: Vec<Entity>,
    pub has_bed: bool,
    pub has_workstation: bool,
    pub panic_timer: Option<Duration>,
    pub reputation: HashMap<Entity, i32>,
}

impl VillagerAi {
    pub fn new(profession: VillagerProfession) -> Self {
        Self {
            profession,
            level: 1,
            home_pos: None,
            work_pos: None,
            bed_pos: None,
            current_activity: VillagerActivity::Idle,
            activity_timer: Duration::ZERO,
            target_pos: None,
            gossip_timer: Duration::ZERO,
            nearby_villagers: Vec::new(),
            has_bed: false,
            has_workstation: false,
            panic_timer: None,
            reputation: HashMap::new(),
        }
    }

    pub fn with_level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }

    pub fn with_home(mut self, pos: BlockPos) -> Self {
        self.home_pos = Some(pos);
        self
    }

    pub fn with_workstation(mut self, pos: BlockPos) -> Self {
        self.work_pos = Some(pos);
        self.has_workstation = true;
        self
    }

    pub fn with_bed(mut self, pos: BlockPos) -> Self {
        self.bed_pos = Some(pos);
        self.has_bed = true;
        self
    }

    pub fn has_bed(&self) -> bool {
        self.has_bed
    }

    pub fn has_workstation(&self) -> bool {
        self.has_workstation
    }

    pub fn can_trade(&self) -> bool {
        self.profession.can_trade()
    }

    pub fn max_level(&self) -> i32 {
        self.profession.max_level()
    }
}

/// Marker component for a villager workstation block.
#[derive(Component, Debug, Clone)]
pub struct VillagerWorkstation {
    pub profession: VillagerProfession,
    pub bound_villager: Option<Entity>,
}

impl VillagerWorkstation {
    pub fn new(profession: VillagerProfession) -> Self {
        Self {
            profession,
            bound_villager: None,
        }
    }
}

/// Marker component for a villager bed block.
#[derive(Component, Debug, Clone)]
pub struct VillagerBed {
    pub bound_villager: Option<Entity>,
}

impl Default for VillagerBed {
    fn default() -> Self {
        Self {
            bound_villager: None,
        }
    }
}

/// A single trade offer.
#[derive(Clone, Debug)]
pub struct TradeOffer {
    pub item1: ItemStack,
    pub item2: Option<ItemStack>,
    pub result: ItemStack,
    pub max_uses: u32,
    pub uses: u32,
    pub reward_exp: bool,
    pub price_multiplier: f32,
    pub xp: i32,
    pub demand: i32,
    pub special: bool,
}

impl TradeOffer {
    pub fn new(item1: ItemStack, result: ItemStack) -> Self {
        Self {
            item1,
            item2: None,
            result,
            max_uses: 12,
            uses: 0,
            reward_exp: true,
            price_multiplier: 0.2,
            xp: 1,
            demand: 0,
            special: false,
        }
    }

    pub fn with_item2(mut self, item: ItemStack) -> Self {
        self.item2 = Some(item);
        self
    }

    pub fn with_max_uses(mut self, max: u32) -> Self {
        self.max_uses = max;
        self
    }

    pub fn with_xp(mut self, xp: i32) -> Self {
        self.xp = xp;
        self
    }

    pub fn is_available(&self) -> bool {
        self.uses < self.max_uses
    }

    pub fn use_offer(&mut self) {
        self.uses += 1;
    }
}

/// Resource holding all trade tables keyed by profession.
#[derive(Resource)]
pub struct TradeTable {
    trades: HashMap<VillagerProfession, Vec<Vec<Vec<TradeOffer>>>>,
}

impl Default for TradeTable {
    fn default() -> Self {
        Self {
            trades: HashMap::new(),
        }
    }
}

impl TradeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_trades(
        &self,
        profession: &VillagerProfession,
        level: i32,
    ) -> Option<&Vec<TradeOffer>> {
        self.trades.get(profession)?.get((level - 1) as usize)
    }

    pub fn register_trades(
        &mut self,
        profession: VillagerProfession,
        level: i32,
        offers: Vec<TradeOffer>,
    ) {
        let levels = self.trades.entry(profession).or_default();
        let idx = (level - 1) as usize;
        while levels.len() <= idx {
            levels.push(Vec::new());
        }
        levels[idx] = offers;
    }
}

/// Event fired when a villager changes activity.
#[derive(Event, Debug, Clone)]
pub struct VillagerActivityChanged {
    pub villager: Entity,
    pub old_activity: VillagerActivity,
    pub new_activity: VillagerActivity,
}

/// Event fired when a villager levels up.
#[derive(Event, Debug, Clone)]
pub struct VillagerLeveledUp {
    pub villager: Entity,
    pub old_level: u8,
    pub new_level: u8,
}

/// Resource for tracking the game time of day (0..24000 ticks).
#[derive(Resource, Debug)]
pub struct TimeOfDay {
    pub time: i64,
}

impl Default for TimeOfDay {
    fn default() -> Self {
        Self { time: 6000 }
    }
}

impl TimeOfDay {
    pub fn hour(&self) -> i64 {
        (self.time % 24000) / 1000
    }

    pub fn is_night(&self) -> bool {
        let t = self.time % 24000;
        t >= 13000 || t < 1000
    }

    pub fn is_day(&self) -> bool {
        !self.is_night()
    }

    pub fn is_work_time(&self) -> bool {
        let t = self.time % 24000;
        t >= 2000 && t < 12000
    }

    pub fn is_bed_time(&self) -> bool {
        let t = self.time % 24000;
        t >= 12000 && t < 13000
    }

    pub fn is_gathering_time(&self) -> bool {
        let t = self.time % 24000;
        t >= 12000 && t < 13000
    }
}

/// System that advances the time of day.
pub fn advance_time_system(mut time: ResMut<TimeOfDay>, time_res: Res<Time>) {
    time.time += (time_res.delta_secs_f64() * 20.0) as i64;
    time.time %= 24000;
}

/// System that manages villager AI behavior.
pub fn villager_ai_system(
    mut villager_query: Query<(Entity, &mut VillagerAi, &Position)>,
    time: Res<TimeOfDay>,
    mut activity_events: EventWriter<VillagerActivityChanged>,
) {
    let delta = Duration::from_secs_f32(0.05);

    for (entity, mut ai, _pos) in &mut villager_query {
        ai.activity_timer += delta;
        ai.gossip_timer += delta;

        let desired = determine_activity(&ai, &time);

        if ai.current_activity != desired {
            let old = ai.current_activity;
            ai.current_activity = desired;
            ai.activity_timer = Duration::ZERO;
            activity_events.send(VillagerActivityChanged {
                villager: entity,
                old_activity: old,
                new_activity: desired,
            });
        }

        match ai.current_activity {
            VillagerActivity::Idle | VillagerActivity::IdleDefault => {
                if ai.activity_timer > Duration::from_secs(10) {
                    ai.target_pos = ai.home_pos.map(|home| {
                        let bp: DVec3 = DVec3::new(home.x as f64, home.y as f64, home.z as f64);
                        let offset = Vec3::new(
                            ((entity.id() as f32 * 7.0).sin() * 4.0) as f64,
                            0.0,
                            ((entity.id() as f32 * 11.0).cos() * 4.0) as f64,
                        );
                        bp + offset
                    });
                }
            }
            VillagerActivity::Working => {
                if ai.has_workstation && ai.activity_timer > Duration::from_secs(5) {
                    ai.target_pos = ai
                        .work_pos
                        .map(|wp| DVec3::new(wp.x as f64, wp.y as f64, wp.z as f64));
                }
            }
            VillagerActivity::Resting => {
                if ai.has_bed && ai.activity_timer > Duration::from_secs(5) {
                    ai.target_pos = ai
                        .bed_pos
                        .map(|bp| DVec3::new(bp.x as f64, bp.y as f64, bp.z as f64));
                }
            }
            VillagerActivity::Gossiping => {
                if ai.gossip_timer > Duration::from_secs(30) && !ai.nearby_villagers.is_empty() {
                    ai.gossip_timer = Duration::ZERO;
                }
            }
            VillagerActivity::Trading => {
                if ai.has_workstation {
                    ai.target_pos = ai
                        .work_pos
                        .map(|wp| DVec3::new(wp.x as f64, wp.y as f64, wp.z as f64));
                }
            }
            VillagerActivity::Fleeing => {
                if let Some(panic_start) = ai.panic_timer {
                    if panic_start > Duration::from_secs(30) {
                        ai.panic_timer = None;
                    }
                }
                ai.target_pos = Some(DVec3::new(_pos.0.x + 16.0, _pos.0.y, _pos.0.z));
            }
            VillagerActivity::Gathering | VillagerActivity::Meeting => {
                if let Some(home) = ai.home_pos {
                    ai.target_pos = Some(DVec3::new(home.x as f64, home.y as f64, home.z as f64));
                }
            }
        }
    }
}

fn determine_activity(ai: &VillagerAi, time: &TimeOfDay) -> VillagerActivity {
    if ai.panic_timer.is_some() {
        return VillagerActivity::Fleeing;
    }

    if time.is_bed_time() {
        if ai.has_bed {
            return VillagerActivity::Resting;
        }
        return VillagerActivity::Idle;
    }

    if time.is_work_time() && ai.can_trade() && ai.has_workstation {
        return VillagerActivity::Working;
    }

    if time.is_gathering_time() {
        return VillagerActivity::Gathering;
    }

    if time.is_night() {
        return VillagerActivity::Idle;
    }

    VillagerActivity::Idle
}

/// System that binds villagers to nearby workstations.
pub fn workstation_binding_system(
    mut villager_query: Query<(Entity, &mut VillagerAi, &Position)>,
    mut workstation_query: Query<
        (Entity, &mut VillagerWorkstation, &Position),
        Without<VillagerAi>,
    >,
) {
    for (v_entity, mut ai, v_pos) in &mut villager_query {
        if ai.has_workstation {
            continue;
        }

        let mut best_distance = f64::MAX;
        let mut best_workstation = None;

        for (ws_entity, mut workstation, ws_pos) in &mut workstation_query {
            if workstation.profession != ai.profession {
                continue;
            }
            if workstation.bound_villager.is_some() {
                continue;
            }

            let distance = (v_pos.0 - ws_pos.0).length_squared();
            if distance < best_distance && distance < 48.0 * 48.0 {
                best_distance = distance;
                best_workstation = Some((ws_entity, ws_pos.0));
            }
        }

        if let Some((ws_entity, ws_pos)) = best_workstation {
            ai.work_pos = Some(BlockPos::new(
                ws_pos.x as i32,
                ws_pos.y as i32,
                ws_pos.z as i32,
            ));
            ai.has_workstation = true;
            if let Ok((_, mut workstation, _)) = workstation_query.get_mut(ws_entity) {
                workstation.bound_villager = Some(v_entity);
            }
        }
    }
}

/// System that binds villagers to nearby beds.
pub fn bed_binding_system(
    mut villager_query: Query<(Entity, &mut VillagerAi, &Position), Without<VillagerBed>>,
    mut bed_query: Query<(Entity, &mut VillagerBed, &Position), Without<VillagerAi>>,
) {
    for (v_entity, mut ai, v_pos) in &mut villager_query {
        if ai.has_bed {
            continue;
        }

        let mut best_distance = f64::MAX;
        let mut best_bed = None;

        for (bed_entity, mut bed, bed_pos) in &mut bed_query {
            if bed.bound_villager.is_some() {
                continue;
            }

            let distance = (v_pos.0 - bed_pos.0).length_squared();
            if distance < best_distance && distance < 48.0 * 48.0 {
                best_distance = distance;
                best_bed = Some((bed_entity, bed_pos.0));
            }
        }

        if let Some((bed_entity, bed_pos)) = best_bed {
            ai.bed_pos = Some(BlockPos::new(
                bed_pos.x as i32,
                bed_pos.y as i32,
                bed_pos.z as i32,
            ));
            ai.has_bed = true;
            if let Ok((_, mut bed, _)) = bed_query.get_mut(bed_entity) {
                bed.bound_villager = Some(v_entity);
            }
        }
    }
}

/// System that moves villagers towards their target position.
pub fn villager_movement_system(mut query: Query<(&mut Position, &VillagerAi)>) {
    for (mut pos, ai) in &mut query {
        if let Some(target) = ai.target_pos {
            let dir = target - pos.0;
            let distance = dir.length();

            if distance > 0.5 {
                let speed = 0.287;
                let movement = dir.normalize() * speed.min(distance);
                pos.0 += movement;
            }
        }
    }
}

pub struct VillagerPlugin;

impl Plugin for VillagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TradeTable>()
            .insert_resource(TimeOfDay::default())
            .add_event::<VillagerActivityChanged>()
            .add_event::<VillagerLeveledUp>()
            .add_systems(
                Update,
                (
                    advance_time_system,
                    villager_ai_system,
                    workstation_binding_system,
                    bed_binding_system,
                    villager_movement_system,
                )
                    .chain(),
            );
    }
}
