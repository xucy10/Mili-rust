use std::collections::HashMap;
use std::time::Duration;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_time::prelude::*;
use valence_math::DVec3;
use valence_protocol::{BlockPos, ItemKind, ItemStack};
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
    trades: HashMap<VillagerProfession, Vec<Vec<TradeOffer>>>,
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
    time.time += (time_res.delta_seconds_f64() * 20.0) as i64;
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
                        let offset = DVec3::new(
                            ((entity.index() as f32 * 7.0).sin() * 4.0) as f64,
                            0.0,
                            ((entity.index() as f32 * 11.0).cos() * 4.0) as f64,
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

        for (ws_entity, workstation, ws_pos) in &mut workstation_query {
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

        for (bed_entity, bed, bed_pos) in &mut bed_query {
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
                let speed: f64 = 0.287;
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
            .add_systems(PreStartup, register_default_trades)
            .add_systems(
                Update,
                (
                    advance_time_system,
                    villager_ai_system,
                    workstation_binding_system,
                    bed_binding_system,
                    villager_movement_system,
                    villager_trade_levelup_system,
                )
                    .chain(),
            );
    }
}

fn register_default_trades(mut trade_table: ResMut<TradeTable>) {
    use valence_protocol::ItemKind;

    let wheat = ItemStack::new(ItemKind::Wheat, 20, None);
    let bread = ItemStack::new(ItemKind::Bread, 1, None);
    let emerald = ItemStack::new(ItemKind::Emerald, 1, None);
    let carrot = ItemStack::new(ItemKind::Carrot, 15, None);
    let potato = ItemStack::new(ItemKind::Potato, 15, None);
    let beetroot = ItemStack::new(ItemKind::Beetroot, 15, None);
    let cake = ItemStack::new(ItemKind::Cake, 1, None);
    let cookie = ItemStack::new(ItemKind::Cookie, 3, None);
    let apple = ItemStack::new(ItemKind::Apple, 4, None);
    let golden_carrot = ItemStack::new(ItemKind::GoldenCarrot, 3, None);
    let glistering_melon = ItemStack::new(ItemKind::GlisteringMelonSlice, 3, None);
    let arrow = ItemStack::new(ItemKind::Arrow, 16, None);
    let bow = ItemStack::new(ItemKind::Bow, 1, None);
    let crossbow = ItemStack::new(ItemKind::Crossbow, 1, None);
    let flint = ItemStack::new(ItemKind::Flint, 10, None);
    let string = ItemStack::new(ItemKind::String, 2, None);
    let feather = ItemStack::new(ItemKind::Feather, 24, None);
    let iron_ingot = ItemStack::new(ItemKind::IronIngot, 4, None);
    let iron_boots = ItemStack::new(ItemKind::IronBoots, 1, None);
    let iron_leggings = ItemStack::new(ItemKind::IronLeggings, 1, None);
    let iron_chestplate = ItemStack::new(ItemKind::IronChestplate, 1, None);
    let iron_helmet = ItemStack::new(ItemKind::IronHelmet, 1, None);
    let iron_sword = ItemStack::new(ItemKind::IronSword, 1, None);
    let iron_axe = ItemStack::new(ItemKind::IronAxe, 1, None);
    let iron_pickaxe = ItemStack::new(ItemKind::IronPickaxe, 1, None);
    let iron_shovel = ItemStack::new(ItemKind::IronShovel, 1, None);
    let diamond = ItemStack::new(ItemKind::Diamond, 1, None);
    let diamond_boots = ItemStack::new(ItemKind::DiamondBoots, 1, None);
    let diamond_leggings = ItemStack::new(ItemKind::DiamondLeggings, 1, None);
    let diamond_chestplate = ItemStack::new(ItemKind::DiamondChestplate, 1, None);
    let diamond_helmet = ItemStack::new(ItemKind::DiamondHelmet, 1, None);
    let _diamond_sword = ItemStack::new(ItemKind::DiamondSword, 1, None);
    let diamond_axe = ItemStack::new(ItemKind::DiamondAxe, 1, None);
    let diamond_pickaxe = ItemStack::new(ItemKind::DiamondPickaxe, 1, None);
    let coal = ItemStack::new(ItemKind::Coal, 15, None);
    let bucket = ItemStack::new(ItemKind::Bucket, 1, None);
    let bell = ItemStack::new(ItemKind::Bell, 1, None);
    let book = ItemStack::new(ItemKind::Book, 12, None);
    let ink_sac = ItemStack::new(ItemKind::InkSac, 5, None);
    let lantern = ItemStack::new(ItemKind::Lantern, 1, None);
    let name_tag = ItemStack::new(ItemKind::NameTag, 1, None);
    let redstone = ItemStack::new(ItemKind::Redstone, 2, None);
    let lapis_lazuli = ItemStack::new(ItemKind::LapisLazuli, 1, None);
    let glowstone = ItemStack::new(ItemKind::GlowstoneDust, 1, None);
    let rotten_flesh = ItemStack::new(ItemKind::RottenFlesh, 32, None);
    let leather = ItemStack::new(ItemKind::Leather, 9, None);
    let saddle = ItemStack::new(ItemKind::Saddle, 6, None);
    let leather_helmet = ItemStack::new(ItemKind::LeatherHelmet, 1, None);
    let leather_chestplate = ItemStack::new(ItemKind::LeatherChestplate, 1, None);
    let leather_leggings = ItemStack::new(ItemKind::LeatherLeggings, 1, None);
    let leather_boots = ItemStack::new(ItemKind::LeatherBoots, 1, None);
    let paper = ItemStack::new(ItemKind::Paper, 24, None);
    let clock = ItemStack::new(ItemKind::Clock, 5, None);
    let glass = ItemStack::new(ItemKind::Glass, 4, None);
    let experience_bottle = ItemStack::new(ItemKind::ExperienceBottle, 1, None);
    let eye_of_ender = ItemStack::new(ItemKind::EnderEye, 1, None);
    let brick = ItemStack::new(ItemKind::Brick, 10, None);
    let stone_bricks = ItemStack::new(ItemKind::StoneBricks, 4, None);
    let chiseled_stone_bricks = ItemStack::new(ItemKind::ChiseledStoneBricks, 1, None);
    let polished_andesite = ItemStack::new(ItemKind::PolishedAndesite, 4, None);
    let quartz = ItemStack::new(ItemKind::Quartz, 5, None);
    let quartz_block = ItemStack::new(ItemKind::QuartzBlock, 1, None);
    let gravel = ItemStack::new(ItemKind::Gravel, 10, None);
    let clay = ItemStack::new(ItemKind::Clay, 10, None);
    let sand = ItemStack::new(ItemKind::Sand, 8, None);

    trade_table.register_trades(
        VillagerProfession::Farmer,
        1,
        vec![
            TradeOffer::new(wheat.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), bread.clone()).with_max_uses(12),
            TradeOffer::new(carrot.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), carrot).with_max_uses(12),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Farmer,
        2,
        vec![
            TradeOffer::new(potato, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), apple.clone()).with_max_uses(4),
            TradeOffer::new(beetroot, emerald.clone()).with_max_uses(16),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Farmer,
        3,
        vec![
            TradeOffer::new(emerald.clone(), cookie.clone()).with_max_uses(12),
            TradeOffer::new(emerald.clone(), cake.clone()).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Farmer,
        4,
        vec![
            TradeOffer::new(emerald.clone(), golden_carrot).with_max_uses(3),
            TradeOffer::new(emerald.clone(), glistering_melon).with_max_uses(3),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Farmer,
        5,
        vec![TradeOffer::new(emerald.clone(), experience_bottle.clone()).with_max_uses(1)],
    );

    trade_table.register_trades(
        VillagerProfession::Cleric,
        1,
        vec![
            TradeOffer::new(coal.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), iron_helmet).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Armorer,
        2,
        vec![
            TradeOffer::new(iron_ingot.clone(), emerald.clone()).with_max_uses(4),
            TradeOffer::new(emerald.clone(), iron_chestplate).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Armorer,
        3,
        vec![
            TradeOffer::new(emerald.clone(), iron_leggings).with_max_uses(4),
            TradeOffer::new(emerald.clone(), iron_boots).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Armorer,
        4,
        vec![
            TradeOffer::new(diamond.clone(), emerald.clone()).with_max_uses(1),
            TradeOffer::new(emerald.clone(), diamond_chestplate).with_max_uses(1),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Armorer,
        5,
        vec![
            TradeOffer::new(emerald.clone(), diamond_helmet).with_max_uses(1),
            TradeOffer::new(emerald.clone(), diamond_leggings).with_max_uses(1),
            TradeOffer::new(emerald.clone(), diamond_boots).with_max_uses(1),
        ],
    );

    trade_table.register_trades(
        VillagerProfession::Weaponsmith,
        1,
        vec![
            TradeOffer::new(coal.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), iron_axe).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Weaponsmith,
        2,
        vec![
            TradeOffer::new(iron_ingot.clone(), emerald.clone()).with_max_uses(4),
            TradeOffer::new(emerald.clone(), iron_sword).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Weaponsmith,
        3,
        vec![TradeOffer::new(emerald.clone(), bell.clone()).with_max_uses(1)],
    );
    trade_table.register_trades(
        VillagerProfession::Weaponsmith,
        5,
        vec![
            TradeOffer::new(diamond.clone(), emerald.clone()).with_max_uses(1),
            TradeOffer::new(emerald.clone(), diamond_axe).with_max_uses(1),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Weaponsmith,
        5,
        vec![TradeOffer::new(emerald.clone(), bell.clone()).with_max_uses(1)],
    );

    trade_table.register_trades(
        VillagerProfession::Toolsmith,
        1,
        vec![
            TradeOffer::new(coal.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), iron_shovel).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Toolsmith,
        2,
        vec![
            TradeOffer::new(iron_ingot.clone(), emerald.clone()).with_max_uses(4),
            TradeOffer::new(emerald.clone(), iron_pickaxe).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Toolsmith,
        3,
        vec![TradeOffer::new(emerald.clone(), bell).with_max_uses(1)],
    );
    trade_table.register_trades(
        VillagerProfession::Toolsmith,
        4,
        vec![
            TradeOffer::new(diamond.clone(), emerald.clone()).with_max_uses(1),
            TradeOffer::new(emerald.clone(), diamond_pickaxe).with_max_uses(1),
        ],
    );

    trade_table.register_trades(
        VillagerProfession::Butcher,
        1,
        vec![
            TradeOffer::new(rotten_flesh.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), rabbit_stew()).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Butcher,
        2,
        vec![
            TradeOffer::new(coal.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), cooked_porkchop()).with_max_uses(8),
        ],
    );

    trade_table.register_trades(
        VillagerProfession::Leatherworker,
        1,
        vec![
            TradeOffer::new(leather.clone(), emerald.clone()).with_max_uses(8),
            TradeOffer::new(emerald.clone(), leather_helmet).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Leatherworker,
        2,
        vec![TradeOffer::new(emerald.clone(), leather_chestplate).with_max_uses(4)],
    );
    trade_table.register_trades(
        VillagerProfession::Leatherworker,
        3,
        vec![
            TradeOffer::new(emerald.clone(), leather_leggings).with_max_uses(4),
            TradeOffer::new(emerald.clone(), saddle).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Leatherworker,
        4,
        vec![TradeOffer::new(emerald.clone(), leather_boots).with_max_uses(4)],
    );

    trade_table.register_trades(
        VillagerProfession::Librarian,
        1,
        vec![
            TradeOffer::new(paper, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), book.clone()).with_max_uses(12),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Librarian,
        2,
        vec![
            TradeOffer::new(book.clone(), emerald.clone()).with_max_uses(8),
            TradeOffer::new(emerald.clone(), lantern).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Librarian,
        3,
        vec![
            TradeOffer::new(ink_sac, emerald.clone()).with_max_uses(8),
            TradeOffer::new(emerald.clone(), glass.clone()).with_max_uses(8),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Librarian,
        4,
        vec![
            TradeOffer::new(emerald.clone(), clock).with_max_uses(4),
            TradeOffer::new(emerald.clone(), experience_bottle.clone()).with_max_uses(1),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Librarian,
        5,
        vec![TradeOffer::new(emerald.clone(), name_tag).with_max_uses(1)],
    );

    trade_table.register_trades(
        VillagerProfession::Cleric,
        1,
        vec![
            TradeOffer::new(rotten_flesh.clone(), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), redstone).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Cleric,
        2,
        vec![
            TradeOffer::new(gold_ingot(), emerald.clone()).with_max_uses(4),
            TradeOffer::new(emerald.clone(), lapis_lazuli).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Cleric,
        3,
        vec![TradeOffer::new(emerald.clone(), eye_of_ender).with_max_uses(4)],
    );
    trade_table.register_trades(
        VillagerProfession::Cleric,
        4,
        vec![TradeOffer::new(emerald.clone(), glowstone).with_max_uses(4)],
    );
    trade_table.register_trades(
        VillagerProfession::Cleric,
        5,
        vec![TradeOffer::new(emerald.clone(), experience_bottle).with_max_uses(1)],
    );

    trade_table.register_trades(
        VillagerProfession::Mason,
        1,
        vec![
            TradeOffer::new(clay, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), brick).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Mason,
        2,
        vec![
            TradeOffer::new(gravel, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), stone_bricks).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Mason,
        3,
        vec![
            TradeOffer::new(sand, emerald.clone()).with_max_uses(8),
            TradeOffer::new(emerald.clone(), polished_andesite).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Mason,
        4,
        vec![TradeOffer::new(emerald.clone(), chiseled_stone_bricks).with_max_uses(4)],
    );
    trade_table.register_trades(
        VillagerProfession::Mason,
        5,
        vec![
            TradeOffer::new(quartz, emerald.clone()).with_max_uses(8),
            TradeOffer::new(emerald.clone(), quartz_block).with_max_uses(4),
        ],
    );

    trade_table.register_trades(
        VillagerProfession::Fisherman,
        1,
        vec![TradeOffer::new(string, emerald.clone()).with_max_uses(16)],
    );
    trade_table.register_trades(
        VillagerProfession::Fisherman,
        2,
        vec![TradeOffer::new(emerald.clone(), bucket).with_max_uses(1)],
    );

    trade_table.register_trades(
        VillagerProfession::Fletcher,
        1,
        vec![
            TradeOffer::new(flint, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), arrow).with_max_uses(16),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Fletcher,
        2,
        vec![
            TradeOffer::new(feather, emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), bow).with_max_uses(4),
        ],
    );
    trade_table.register_trades(
        VillagerProfession::Fletcher,
        3,
        vec![TradeOffer::new(emerald.clone(), crossbow).with_max_uses(4)],
    );

    trade_table.register_trades(
        VillagerProfession::Shepherd,
        1,
        vec![
            TradeOffer::new(wool(ItemKind::WhiteWool), emerald.clone()).with_max_uses(16),
            TradeOffer::new(emerald.clone(), wool(ItemKind::WhiteWool)).with_max_uses(16),
        ],
    );
}

fn rabbit_stew() -> ItemStack {
    ItemStack::new(ItemKind::RabbitStew, 1, None)
}

fn cooked_porkchop() -> ItemStack {
    ItemStack::new(ItemKind::CookedPorkchop, 5, None)
}

fn gold_ingot() -> ItemStack {
    ItemStack::new(ItemKind::GoldIngot, 3, None)
}

fn wool(kind: ItemKind) -> ItemStack {
    ItemStack::new(kind, 1, None)
}

fn villager_trade_levelup_system(
    mut villager_query: Query<(Entity, &mut VillagerAi)>,
    mut level_events: EventWriter<VillagerLeveledUp>,
    trade_table: Res<TradeTable>,
) {
    for (entity, mut ai) in &mut villager_query {
        if !ai.can_trade() {
            continue;
        }

        let max = ai.max_level() as u8;
        if ai.level >= max {
            continue;
        }

        let xp_threshold = match ai.level {
            1 => 10,
            2 => 30,
            3 => 70,
            4 => 150,
            _ => 250,
        };

        let trades = trade_table.get_trades(&ai.profession, ai.level as i32);
        if let Some(trades) = trades {
            let total_uses: u32 = trades.iter().map(|t| t.uses as u32).sum();
            if total_uses >= xp_threshold as u32 {
                let old = ai.level;
                ai.level += 1;
                level_events.send(VillagerLeveledUp {
                    villager: entity,
                    old_level: old,
                    new_level: ai.level,
                });
            }
        }
    }
}