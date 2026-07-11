# Mili-rust 原版机制实现计划

## 概述

基于 valence 框架，实现完整的 Minecraft 原版游戏机制，包括红石、生物AI、村民、漏斗、作物生长、世界保存等系统。

**设计原则**：
- 遵循 valence 的 Bevy ECS 架构
- 保持与现有代码风格一致
- 优先正确性，后优化性能
- 修补 Mojang 原版代码的杂乱设计

## 架构设计

### 1. 新增 Crate 结构

```
crates/
├── valence_vanilla/           # 原版机制核心库
│   ├── src/
│   │   ├── lib.rs
│   │   ├── block_update.rs    # 方块更新传播系统
│   │   ├── tick_schedule.rs   # Tick 调度器
│   │   ├── redstone/          # 红石系统
│   │   │   ├── mod.rs
│   │   │   ├── wire.rs        # 红石线
│   │   │   ├── torch.rs       # 红石火把
│   │   │   ├── repeater.rs    # 中继器
│   │   │   ├── comparator.rs  # 比较器
│   │   │   ├── piston.rs      # 活塞
│   │   │   ├── lamp.rs        # 红石灯
│   │   │   ├── dust.rs        # 红石粉尘
│   │   │   └── signal.rs      # 信号传播逻辑
│   │   ├── hopper.rs          # 漏斗系统
│   │   ├── crop.rs            # 作物生长系统
│   │   ├── physics.rs         # 物理引擎（重力、碰撞）
│   │   ├── entity_ai/         # 生物AI系统
│   │   │   ├── mod.rs
│   │   │   ├── pathfinding.rs # A* 寻路
│   │   │   ├── behavior.rs    # 行为树
│   │   │   ├── memory.rs      # 实体记忆
│   │   │   └── perception.rs  # 实体感知
│   │   ├── villager.rs        # 村民系统
│   │   └── world_save.rs      # 世界保存系统
│   └── Cargo.toml
│
├── valence_world/             # 世界管理
│   ├── src/
│   │   ├── lib.rs
│   │   ├── level_dat.rs       # level.dat 读写
│   │   ├── world_manager.rs   # 世界管理器
│   │   └── save_system.rs     # 保存系统
│   └── Cargo.toml
```

### 2. 系统调度架构

```rust
// 新增的 Schedule/SystemSet
pub struct BlockUpdateSet;      // 方块更新传播
pub struct TickScheduleSet;     // Tick 调度
pub struct RedstoneSet;         // 红石逻辑
pub struct PhysicsSet;          // 物理计算
pub struct EntityAiSet;         // 实体AI
pub struct WorldSaveSet;        // 世界保存

// 调度顺序
PreUpdate
  → EventLoopPreUpdate
  → EventLoopUpdate
  → EventLoopPostUpdate
Update
  → BlockUpdateSet (方块更新传播)
  → TickScheduleSet (随机tick、计划tick)
  → RedstoneSet (红石信号计算)
  → PhysicsSet (重力、碰撞)
  → EntityAiSet (生物AI、寻路)
  → CropGrowthSet (作物生长)
  → HopperSet (漏斗传输)
PostUpdate
  → UpdateLayersPreClientSet
  → UpdateClientsSet
  → WorldSaveSet (世界保存检查)
  → FlushPacketsSet
```

## 详细设计

### 1. 方块更新传播系统

#### 数据结构

```rust
/// 方块更新类型
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlockUpdateType {
    /// 邻居方块变化（放置/破坏）
    NeighborUpdate,
    /// 计划tick（中继器延迟、熔岩流动等）
    ScheduledTick,
    /// 随机tick（作物生长、树叶衰减等）
    RandomTick,
    /// 红石信号更新
    RedstoneUpdate,
}

/// 方块更新事件
#[derive(Event)]
pub struct BlockUpdateEvent {
    pub pos: BlockPos,
    pub old_state: BlockState,
    pub new_state: BlockState,
    pub update_type: BlockUpdateType,
}

/// 计划tick条目
#[derive(Clone, Copy)]
pub struct ScheduledTick {
    pub pos: BlockPos,
    pub tick_type: BlockTickType,
    pub delay: u32,  // 延迟ticks
    pub priority: i32,
}

/// 区块tick调度器
#[derive(Resource)]
pub struct ChunkTickScheduler {
    /// 计划tick队列
    scheduled_ticks: Vec<ScheduledTick>,
    /// 随机tick计数器
    random_tick_counter: u32,
}
```

#### 核心方法

```rust
impl ChunkLayer {
    /// 设置方块并触发更新传播
    pub fn set_block_with_update(
        &mut self,
        pos: BlockPos,
        state: BlockState,
    ) -> Option<Block> {
        let old_state = self.block(pos).map(|b| b.state);
        let result = self.set_block(pos, state);
        
        // 触发邻居更新
        if let Some(old) = old_state {
            self.notify_neighbors(pos, old, state);
        }
        
        result
    }
    
    /// 通知邻居方块
    fn notify_neighbors(
        &mut self,
        pos: BlockPos,
        old_state: BlockState,
        new_state: BlockState,
    ) {
        // 6个方向的邻居
        for dir in Direction::ALL {
            let neighbor_pos = pos.offset(dir);
            if let Some(neighbor_state) = self.block(neighbor_pos) {
                // 调用方块的 onNeighborUpdate 回调
                self.on_neighbor_update(neighbor_pos, pos, neighbor_state);
            }
        }
    }
    
    /// 调度计划tick
    pub fn schedule_tick(
        &mut self,
        pos: BlockPos,
        tick_type: BlockTickType,
        delay: u32,
        priority: i32,
    );
    
    /// 执行随机tick
    fn process_random_ticks(&mut self, chunks: &[ChunkPos], ticks_per_chunk: u32);
}
```

### 2. Tick 调度器

#### 数据结构

```rust
/// Tick 调度器资源
#[derive(Resource)]
pub struct TickScheduler {
    /// 服务器当前tick
    current_tick: u64,
    /// 计划tick队列（按时间排序）
    scheduled_ticks: BinaryHeap<ScheduledTickEntry>,
    /// 每tick处理的随机tick数
    random_ticks_per_chunk: u32,
    /// 游戏规则：随机tick是否启用
    do_random_tick: bool,
}

#[derive(Clone, Eq, PartialEq)]
struct ScheduledTickEntry {
    tick: ScheduledTick,
    execute_at: u64,
}

impl Ord for ScheduledTickEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // 反向比较，使 BinaryHeap 成为最小堆
        other.execute_at.cmp(&self.execute_at)
            .then_with(|| self.tick.priority.cmp(&other.tick.priority))
    }
}
```

#### 系统实现

```rust
/// Tick 调度系统
pub fn tick_scheduler_system(
    mut scheduler: ResMut<TickScheduler>,
    mut chunk_layers: Query<&mut ChunkLayer>,
    time: Res<Time>,
) {
    scheduler.current_tick += 1;
    
    // 处理计划ticks
    while let Some(entry) = scheduler.scheduled_ticks.peek() {
        if entry.execute_at > scheduler.current_tick {
            break;
        }
        
        let entry = scheduler.scheduled_ticks.pop().unwrap();
        // 执行tick回调
        execute_scheduled_tick(&mut chunk_layers, &entry.tick);
    }
    
    // 处理随机ticks（如果启用）
    if scheduler.do_random_tick {
        process_random_ticks(&mut chunk_layers, scheduler.random_ticks_per_chunk);
    }
}
```

### 3. 红石系统

#### 红石信号传播

```rust
/// 红石信号强度
pub type RedstoneStrength = u8; // 0-15

/// 红石更新类型
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RedstoneUpdate {
    /// 信号源变化（红石火把、拉杆等）
    SourceUpdate,
    /// 信号传播（红石线、中继器等）
    PropagationUpdate,
    /// 邻居变化
    NeighborUpdate,
}

/// 红石信号查询结果
#[derive(Clone, Copy)]
pub struct RedstoneSignal {
    /// 信号强度
    pub strength: RedstoneStrength,
    /// 信号来源方向
    pub from_direction: Option<Direction>,
    /// 是否是强信号
    pub is_strong: bool,
}

/// 红石元件 trait
pub trait RedstoneComponent: Component {
    /// 获取当前输出信号强度
    fn get_output_strength(&self, facing: Direction) -> RedstoneStrength;
    
    /// 获取当前输入信号强度
    fn get_input_strength(&self, facing: Direction) -> RedstoneStrength;
    
    /// 更新信号状态
    fn update_signal(&mut self, inputs: &[RedstoneSignal]) -> bool;
    
    /// 是否需要计划tick
    fn needs_scheduled_tick(&self) -> bool;
}
```

#### 红石线实现

```rust
/// 红石线组件
#[derive(Component)]
pub struct RedstoneWire {
    /// 当前信号强度
    pub power: RedstoneStrength,
    /// 连接方向
    pub connections: [WireConnection; 4],
    /// 是否正在更新
    updating: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WireConnection {
    None,
    Side,      // 侧面连接
    Up,        // 向上连接
    Down,      // 向下连接
}

impl RedstoneComponent for RedstoneWire {
    fn get_output_strength(&self, facing: Direction) -> RedstoneStrength {
        self.power
    }
    
    fn get_input_strength(&self, facing: Direction) -> RedstoneStrength {
        self.power
    }
    
    fn update_signal(&mut self, inputs: &[RedstoneSignal]) -> bool {
        let max_input = inputs.iter()
            .map(|s| s.strength)
            .max()
            .unwrap_or(0);
        
        let new_power = max_input.saturating_sub(1);
        
        if self.power != new_power {
            self.power = new_power;
            true  // 需要通知邻居
        } else {
            false
        }
    }
}
```

#### 红石中继器

```rust
/// 中继器组件
#[derive(Component)]
pub struct RedstoneRepeater {
    /// 延迟 (1-4 ticks)
    pub delay: u8,
    /// 锁定状态
    pub locked: bool,
    /// 模式 (false=延迟, true=锁定)
    pub mode: RepeaterMode,
    /// 输入信号强度
    pub input_power: RedstoneStrength,
    /// 输出信号强度
    pub output_power: RedstoneStrength,
    /// 上次更新的tick
    last_tick: u64,
}

impl RedstoneComponent for RedstoneRepeater {
    fn get_output_strength(&self, facing: Direction) -> RedstoneStrength {
        if facing == self.facing() {
            self.output_power
        } else {
            0
        }
    }
    
    fn update_signal(&mut self, inputs: &[RedstoneSignal]) -> bool {
        // 中继器逻辑：
        // 1. 从背面输入
        // 2. 经过延迟后输出到正面
        // 3. 侧面输入用于锁定
        // ...
        true
    }
}
```

### 4. 漏斗系统

```rust
/// 漏斗组件
#[derive(Component)]
pub struct Hopper {
    /// 传输冷却 (0-8 ticks)
    pub transfer_cooldown: u8,
    /// 是否启用
    pub enabled: bool,
    /// 输出方向
    pub output_direction: Direction,
}

/// 漏斗系统
pub fn hopper_system(
    mut hoppers: Query<(&mut Hopper, &Position, &mut Inventory)>,
    mut chunk_layers: Query<&mut ChunkLayer>,
    tick_scheduler: Res<TickScheduler>,
) {
    for (mut hopper, pos, mut inventory) in &mut hoppers {
        if hopper.transfer_cooldown > 0 {
            hopper.transfer_cooldown -= 1;
            continue;
        }
        
        // 尝试从上方拉取物品
        let above_pos = BlockPos::new(pos.x as i32, pos.y as i32 + 1, pos.z as i32);
        if let Some(mut above_inv) = get_inventory_at(&chunk_layers, above_pos) {
            try_pull_items(&mut above_inv, &mut inventory);
        }
        
        // 尝试向输出方向推出物品
        let output_pos = pos.offset(hopper.output_direction);
        if let Some(mut output_inv) = get_inventory_at(&chunk_layers, output_pos) {
            try_push_items(&mut inventory, &mut output_inv);
        }
        
        hopper.transfer_cooldown = 8;
    }
}
```

### 5. 作物生长系统

```rust
/// 作物类型
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CropType {
    Wheat,
    Carrot,
    Potato,
    Beetroot,
    Pumpkin,
    Melon,
    SugarCane,
    Bamboo,
    Cocoa,
}

/// 作物组件
#[derive(Component)]
pub struct Crop {
    pub crop_type: CropType,
    pub age: u8,          // 当前生长阶段
    pub max_age: u8,      // 最大生长阶段
    pub growth_chance: f64, // 每tick生长概率
    pub needs_water: bool,
    pub needs_light: bool,
    pub min_light_level: u8,
}

/// 作物生长系统
pub fn crop_growth_system(
    mut crops: Query<(&mut Crop, &Position, &mut BlockState)>,
    mut chunk_layers: Query<&mut ChunkLayer>,
    tick_scheduler: Res<TickScheduler>,
    random: Res<SmallRng>,
) {
    for (mut crop, pos, mut state) in &mut crops {
        // 检查生长条件
        if !check_growth_conditions(&crop, &chunk_layers, *pos) {
            continue;
        }
        
        // 计算生长概率
        let growth_prob = calculate_growth_probability(&crop, &chunk_layers, *pos);
        
        // 随机tick生长
        if random.gen_bool(growth_prob) {
            crop.age = (crop.age + 1).min(crop.max_age);
            
            // 更新方块状态
            let new_state = state.set(PropName::Age, PropValue::from_u16(crop.age as u16));
            chunk_layers.set_block(*pos, new_state);
        }
    }
}

fn check_growth_conditions(crop: &Crop, layers: &ChunkLayer, pos: BlockPos) -> bool {
    // 检查光照
    if crop.needs_light {
        let light_level = get_sky_light(layers, pos) + get_block_light(layers, pos);
        if light_level < crop.min_light_level {
            return false;
        }
    }
    
    // 检查水源
    if crop.needs_water {
        if !has_water_nearby(layers, pos) {
            return false;
        }
    }
    
    // 检查土壤
    let below_pos = BlockPos::new(pos.x, pos.y - 1, pos.z);
    if let Some(below) = layers.block(below_pos) {
        is_fertile_soil(below.state)
    } else {
        false
    }
}
```

### 6. 物理引擎

```rust
/// 物体组件
#[derive(Component)]
pub struct PhysicsBody {
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub mass: f32,
    pub drag: f32,
    pub gravity_multiplier: f32,
    pub on_ground: bool,
    pub collides_with_blocks: bool,
    pub collides_with_entities: bool,
}

/// 物理系统
pub fn physics_system(
    mut bodies: Query<(&mut PhysicsBody, &mut Position, &Hitbox)>,
    chunk_layers: Query<&ChunkLayer>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    
    for (mut body, mut pos, hitbox) in &mut bodies {
        // 应用重力
        body.velocity.y -= 9.81 * body.gravity_multiplier * dt;
        
        // 应用加速度
        body.velocity += body.acceleration * dt;
        
        // 应用阻力
        body.velocity *= 1.0 - body.drag * dt;
        
        // 计算新位置
        let new_pos = pos.0 + body.velocity * dt as f64;
        
        // 碰撞检测
        if body.collides_with_blocks {
            let collision = check_block_collision(
                &chunk_layers,
                pos.0,
                new_pos,
                hitbox.0,
            );
            
            if collision.hit {
                // 调整位置和速度
                pos.0 = collision.position;
                body.velocity = collision.velocity;
                body.on_ground = collision.on_ground;
            } else {
                pos.0 = new_pos;
            }
        } else {
            pos.0 = new_pos;
        }
    }
}
```

### 7. 生物AI系统

#### 寻路算法

```rust
/// A* 寻路节点
#[derive(Clone, Eq, PartialEq)]
struct PathNode {
    pos: BlockPos,
    g_cost: u32,  // 从起点到当前节点的代价
    h_cost: u32,  // 从当前节点到终点的启发式估计
    parent: Option<BlockPos>,
}

impl PathNode {
    fn f_cost(&self) -> u32 {
        self.g_cost + self.h_cost
    }
}

/// 寻路上下文
pub struct PathfindingContext<'a> {
    chunk_layer: &'a ChunkLayer,
    start: BlockPos,
    end: BlockPos,
    max_iterations: u32,
    allow_swim: bool,
    allow_climb: bool,
    entity_height: f32,
    entity_width: f32,
}

/// A* 寻路算法
pub fn find_path(ctx: &PathfindingContext) -> Option<Vec<BlockPos>> {
    let mut open_set: BinaryHeap<PathNode> = BinaryHeap::new();
    let mut closed_set: HashSet<BlockPos> = HashSet::new();
    let mut g_costs: HashMap<BlockPos, u32> = HashMap::new();
    
    let start_node = PathNode {
        pos: ctx.start,
        g_cost: 0,
        h_cost: heuristic(ctx.start, ctx.end),
        parent: None,
    };
    
    g_costs.insert(ctx.start, 0);
    open_set.push(start_node);
    
    let mut iterations = 0;
    
    while let Some(current) = open_set.pop() {
        if current.pos == ctx.end {
            return reconstruct_path(&current);
        }
        
        if iterations >= ctx.max_iterations {
            return None;
        }
        iterations += 1;
        
        closed_set.insert(current.pos);
        
        // 检查所有邻居
        for neighbor_pos in get_walkable_neighbors(ctx, current.pos) {
            if closed_set.contains(&neighbor_pos) {
                continue;
            }
            
            let tentative_g = current.g_cost + move_cost(current.pos, neighbor_pos);
            
            if let Some(&best_g) = g_costs.get(&neighbor_pos) {
                if tentative_g >= best_g {
                    continue;
                }
            }
            
            g_costs.insert(neighbor_pos, tentative_g);
            
            let neighbor_node = PathNode {
                pos: neighbor_pos,
                g_cost: tentative_g,
                h_cost: heuristic(neighbor_pos, ctx.end),
                parent: Some(current.pos),
            };
            
            open_set.push(neighbor_node);
        }
    }
    
    None
}

fn heuristic(a: BlockPos, b: BlockPos) -> u32 {
    // 曼哈顿距离
    ((a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs()) as u32
}
```

#### 行为树

```rust
/// 行为树节点状态
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

/// 行为树节点 trait
pub trait BehaviorNode: Send + Sync {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus;
}

/// 行为上下文
pub struct BehaviorContext<'a> {
    pub world: &'a World,
    pub entity: Entity,
    pub position: Position,
    pub target: Option<Entity>,
    pub memory: &'a mut EntityMemory,
}

/// 组合节点：顺序执行
pub struct Sequence {
    children: Vec<Box<dyn BehaviorNode>>,
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

/// 组合节点：选择执行
pub struct Selector {
    children: Vec<Box<dyn BehaviorNode>>,
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

/// 装饰器节点：条件检查
pub struct Condition {
    condition: Box<dyn Fn(&BehaviorContext) -> bool>,
    child: Box<dyn BehaviorNode>,
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

/// 叶子节点：移动到目标
pub struct MoveToTarget;

impl BehaviorNode for MoveToTarget {
    fn tick(&self, entity: Entity, ctx: &mut BehaviorContext) -> BehaviorStatus {
        if let Some(target) = ctx.target {
            let target_pos = ctx.world.get::<Position>(target);
            let path = find_path(&PathfindingContext {
                chunk_layer: ctx.world.get::<ChunkLayer>(/* ... */),
                start: ctx.position.0.into(),
                end: target_pos.0.into(),
                max_iterations: 1000,
                allow_swim: false,
                allow_climb: false,
                entity_height: 1.8,
                entity_width: 0.6,
            });
            
            if let Some(path) = path {
                // 存储路径到记忆中
                ctx.memory.current_path = path;
                BehaviorStatus::Running
            } else {
                BehaviorStatus::Failure
            }
        } else {
            BehaviorStatus::Failure
        }
    }
}
```

### 8. 村民系统

```rust
/// 村民职业
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Nitwit,
    Shepherd,
    Toolsmith,
    Weaponsmith,
}

/// 村民AI组件
#[derive(Component)]
pub struct VillagerAI {
    pub profession: VillagerProfession,
    pub level: u8,  // 1-5
    pub home_pos: Option<BlockPos>,  // 床的位置
    pub work_pos: Option<BlockPos>,  // 工作站的位置
    pub gossip: Vec<Gossip>,
    pub reputation: HashMap<Entity, i32>,
    pub current_activity: VillagerActivity,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VillagerActivity {
    Idle,
    Working,
    Resting,
    Gossiping,
    Trading,
    Fleeing,
}

/// 交易表
#[derive(Resource)]
pub struct TradeTable {
    trades: HashMap<VillagerProfession, Vec<TradeOffer>>,
}

pub struct TradeOffer {
    pub item1: ItemStack,
    pub item2: Option<ItemStack>,
    pub result: ItemStack,
    pub max_uses: u32,
    pub uses: u32,
    pub reward_exp: bool,
    pub price_multiplier: f32,
}

/// 村民AI系统
pub fn villager_ai_system(
    mut villagers: Query<(&mut VillagerAI, &mut Position, &mut PhysicsBody)>,
    players: Query<(&Position, &Inventory)>,
    chunk_layers: Query<&ChunkLayer>,
    trade_table: Res<TradeTable>,
    time: Res<Time>,
) {
    for (mut ai, mut pos, mut body) in &mut villagers {
        // 更新AI状态
        update_villager_activity(&mut ai, &players, &chunk_layers);
        
        // 执行当前活动的AI行为
        match ai.current_activity {
            VillagerActivity::Idle => idle_behavior(&mut ai, &mut pos, &mut body),
            VillagerActivity::Working => work_behavior(&mut ai, &mut pos, &mut body, &trade_table),
            VillagerActivity::Resting => rest_behavior(&mut ai, &mut pos),
            VillagerActivity::Gossiping => gossip_behavior(&mut ai, &mut pos),
            VillagerActivity::Trading => { /* 交易由玩家交互触发 */ },
            VillagerActivity::Fleeing => flee_behavior(&mut ai, &mut pos, &mut body),
        }
    }
}
```

### 9. 世界保存系统

```rust
/// 世界保存管理器
#[derive(Resource)]
pub struct WorldSaveManager {
    /// 保存目录路径
    save_path: PathBuf,
    /// 上次保存时间
    last_save_time: Instant,
    /// 自动保存间隔
    auto_save_interval: Duration,
    /// 待保存的区块
    dirty_chunks: HashSet<ChunkPos>,
    /// 保存线程池
    save_thread_pool: ThreadPool,
}

impl WorldSaveManager {
    pub fn new(save_path: PathBuf) -> Self {
        Self {
            save_path,
            last_save_time: Instant::now(),
            auto_save_interval: Duration::from_secs(300), // 5分钟
            dirty_chunks: HashSet::new(),
            save_thread_pool: ThreadPool::new(4),
        }
    }
    
    /// 标记区块为脏
    pub fn mark_dirty(&mut self, pos: ChunkPos) {
        self.dirty_chunks.insert(pos);
    }
    
    /// 执行保存
    pub fn save(&mut self, chunk_layers: &ChunkLayer) -> Result<(), SaveError> {
        // 1. 保存 level.dat
        self.save_level_dat()?;
        
        // 2. 保存脏区块
        for pos in &self.dirty_chunks {
            self.save_chunk(chunk_layers, *pos)?;
        }
        
        self.dirty_chunks.clear();
        self.last_save_time = Instant::now();
        
        Ok(())
    }
    
    /// 保存单个区块
    fn save_chunk(
        &self,
        layer: &ChunkLayer,
        pos: ChunkPos,
    ) -> Result<(), SaveError> {
        let chunk = layer.chunk(pos)
            .ok_or(SaveError::ChunkNotLoaded(pos))?;
        
        // 转换为 Anvil 格式
        let anvil_data = convert_to_anvil(chunk);
        
        // 写入 .mca 文件
        let region_x = pos.x.div_euclid(32);
        let region_z = pos.z.div_euclid(32);
        let region_path = self.save_path
            .join("region")
            .join(format!("r.{region_x}.{region_z}.mca"));
        
        let mut region = RegionFolder::new(&region_path);
        region.set_chunk(pos.x, pos.z, &anvil_data)?;
        
        Ok(())
    }
    
    /// 检查是否需要自动保存
    fn check_auto_save(&mut self, chunk_layers: &ChunkLayer) {
        if self.last_save_time.elapsed() >= self.auto_save_interval {
            if let Err(e) = self.save(chunk_layers) {
                error!("Auto save failed: {:?}", e);
            }
        }
    }
}

/// 保存系统
pub fn world_save_system(
    mut save_manager: ResMut<WorldSaveManager>,
    chunk_layers: Query<&ChunkLayer>,
    // 服务器关闭事件
    mut shutdown_events: EventReader<ShutdownEvent>,
) {
    // 检查自动保存
    for layer in &chunk_layers {
        save_manager.check_auto_save(layer);
    }
    
    // 处理关闭事件
    for _ in shutdown_events.read() {
        for layer in &chunk_layers {
            if let Err(e) = save_manager.save(layer) {
                error!("Shutdown save failed: {:?}", e);
            }
        }
    }
}
```

### 10. level.dat 读写

```rust
/// level.dat 数据结构
#[derive(Debug, Clone)]
pub struct LevelDat {
    pub version: LevelVersion,
    pub data: LevelData,
}

#[derive(Debug, Clone)]
pub struct LevelVersion {
    pub id: i32,
    pub name: String,
    pub snapshot: bool,
    pub major: u8,
}

#[derive(Debug, Clone)]
pub struct LevelData {
    pub allow_commands: bool,
    pub border_center_x: f64,
    pub border_center_z: f64,
    pub border_damage_per_block: f64,
    pub border_safe_zone: f64,
    pub border_size: f64,
    pub border_warning_blocks: i32,
    pub border_warning_time: i32,
    pub clear_weather_time: i32,
    pub customized: bool,
    pub data_version: i32,
    pub day_time: i64,
    pub difficulty: u8,
    pub difficulty_locked: bool,
    pub dimension_data: HashMap<String, DimensionData>,
    pub enabled_features: Vec<String>,
    pub game_type: u32,
    pub generator_name: String,
    pub generator_options: Compound,
    pub generator_version: i32,
    pub hardcore: bool,
    pub initialized: bool,
    pub last_played: i64,
    pub level_name: String,
    pub lightning_time: i32,
    pub raining: bool,
    pub rain_time: i32,
    pub scheduled_ticks: Vec<ScheduledTickNbt>,
    pub spawning: SpawningData,
    pub spawn_x: i32,
    pub spawn_y: i32,
    pub spawn_z: i32,
    pub thundering: bool,
    pub thunder_time: i32,
    pub time: i64,
    pub version: LevelVersion,
    pub world_settings: WorldSettings,
}

impl LevelDat {
    /// 从文件读取
    pub fn read(path: &Path) -> Result<Self, LevelDatError> {
        let mut file = File::open(path)?;
        let mut decoder = GzDecoder::new(&mut file);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf)?;
        
        let (nbt, _) = valence_nbt::from_binary(&mut buf.as_slice())?;
        Self::from_nbt(&nbt)
    }
    
    /// 写入文件
    pub fn write(&self, path: &Path) -> Result<(), LevelDatError> {
        let nbt = self.to_nbt();
        
        let file = File::create(path)?;
        let mut encoder = GzEncoder::new(file, flate2::Compression::best());
        valence_nbt::to_binary(&nbt, &mut encoder, "")?;
        
        Ok(())
    }
    
    fn from_nbt(nbt: &Compound) -> Result<Self, LevelDatError> {
        // 解析 NBT 数据
        // ...
    }
    
    fn to_nbt(&self) -> Compound {
        // 转换为 NBT 格式
        // ...
    }
}
```

## 实现阶段

### 阶段1：基础系统 (1-2周)

1. **创建 crate 结构**
   - 创建 `valence_vanilla` 和 `valence_world` crate
   - 配置 Cargo.toml 依赖

2. **方块更新传播系统**
   - 实现 `set_block_with_update`
   - 实现邻居通知机制
   - 实现计划tick调度

3. **Tick 调度器**
   - 实现 `TickScheduler` 资源
   - 实现随机tick处理
   - 集成到游戏循环

### 阶段2：红石系统 (2-3周)

1. **红石基础**
   - 实现红石信号传播
   - 实现红石线
   - 实现红石火把

2. **红石元件**
   - 实现中继器
   - 实现比较器
   - 实现红石灯

3. **高级红石**
   - 实现活塞
   - 实现发射器/投掷器
   - 优化信号传播性能

### 阶段3：游戏机制 (2-3周)

1. **漏斗系统**
   - 实现物品传输
   - 实现计时器
   - 实现与箱子的交互

2. **作物生长**
   - 实现随机tick生长
   - 实现骨粉加速
   - 实现生长条件检查

3. **物理引擎**
   - 实现重力
   - 实现碰撞检测
   - 实现下落方块

### 阶段4：AI系统 (3-4周)

1. **寻路算法**
   - 实现 A* 寻路
   - 实现路径缓存
   - 优化性能

2. **行为树**
   - 实现基础行为树节点
   - 实现组合节点
   - 实现叶子节点

3. **实体AI**
   - 实现基础AI组件
   - 实现感知系统
   - 实现记忆系统

### 阶段5：村民系统 (2-3周)

1. **村民AI**
   - 实现职业系统
   - 实现工作站绑定
   - 实现床绑定

2. **交易系统**
   - 实现交易表
   - 实现交易UI
   - 实现价格调整

3. **村民行为**
   - 实现日常工作
   - 实现社交行为
   - 实现恐惧反应

### 阶段6：世界保存 (1-2周)

1. **level.dat**
   - 实现读取
   - 实现写入
   - 实现默认值

2. **保存系统**
   - 实现自动保存
   - 实现关闭保存
   - 实现保存命令

3. **加载系统**
   - 实现世界加载
   - 实现区块加载
   - 实现玩家重生点

## 关键设计决策

### 1. 为什么选择 ECS 架构？

- 与 valence 框架一致
- 支持高性能并行处理
- 易于扩展和维护
- 数据驱动的设计

### 2. 为什么修补 Mojang 代码？

原版代码的问题：
- Java 代码风格混乱
- 过多的继承层次
- 不一致的命名约定
- 性能问题

Rust 实现的优势：
- 更清晰的代码结构
- 更好的性能
- 更强的类型安全
- 更容易维护

### 3. 性能优化策略

- 使用位运算优化红石信号传播
- 实现空间分区（BVH）加速碰撞检测
- 使用线程池并行处理AI
- 实现路径缓存减少重复计算

## 测试计划

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_block_update_propagation() {
        // 测试方块更新传播
    }
    
    #[test]
    fn test_redstone_signal_propagation() {
        // 测试红石信号传播
    }
    
    #[test]
    fn test_hopper_transfer() {
        // 测试漏斗传输
    }
    
    #[test]
    fn test_crop_growth() {
        // 测试作物生长
    }
    
    #[test]
    fn test_pathfinding() {
        // 测试寻路算法
    }
}
```

### 集成测试

```rust
#[test]
fn test_redstone_circuit() {
    // 测试完整红石电路
}

#[test]
fn test_villager_trading() {
    // 测试村民交易
}

#[test]
fn test_world_save_load() {
    // 测试世界保存加载
}
```

### 性能测试

```rust
#[bench]
fn bench_redstone_propagation(b: &mut Bencher) {
    // 基准测试红石信号传播
}

#[bench]
fn bench_pathfinding(b: &mut Bencher) {
    // 基准测试寻路算法
}
```

## 示例程序

```rust
// examples/redstone_demo.rs
// 红石演示

// examples/farm_demo.rs
// 农场演示

// examples/village_demo.rs
// 村庄演示

// examples/world_save_demo.rs
// 世界保存演示
```

## 总结

这个实现计划涵盖了所有要求的原版机制，并提供了清晰的架构设计和实现路径。通过分阶段实施，可以逐步构建完整的 Minecraft 服务器功能。

关键优势：
1. 与 valence 框架完美集成
2. 清晰的模块化设计
3. 完整的测试覆盖
4. 良好的性能优化
5. 修补了 Mojang 原版代码的问题

预计总工期：12-16周
