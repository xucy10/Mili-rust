# Mili-rust 架构设计评审报告

> 评审范围：基于 Valence 框架的 Rust Minecraft 服务端（MC 26.2）
> 评审日期：2026-07-16
> 代码基线：`main` 分支最新提交

---

## 一、总体架构概览

### 1.1 技术栈与框架选型

| 层级 | 技术选型 | 评估 |
|------|---------|------|
| 语言 | Rust | ✅ 高性能、内存安全，适合服务端 |
| ECS 框架 | Bevy ECS 0.14 | ✅ 成熟、多线程、数据驱动 |
| 基础框架 | Valence 0.2.0-alpha | ⚠️ 尚处 alpha，API 不稳定 |
| 协议层 | valence_protocol (自研) | ✅ 完整覆盖 26.2 协议 |
| 网络层 | tokio + 自定义 packet_io | ✅ 异步 I/O，合理 |

**总体评价**：选型方向正确。Bevy ECS + Rust 的组合为后续高并发、多线程区域化（Regionized）调度打下了好基础。但 Valence 本身尚未成熟，需关注 upstream breaking changes。

### 1.2 Crate 划分

```
crates/
├── valence_protocol/      # 网络协议 ✅ 独立清晰
├── valence_server/        # 服务器核心（Layer、Client、事件循环）✅
├── valence_entity/        # 实体组件（Position、Velocity、Hitbox）✅
├── valence_inventory/     # 背包系统 ✅
├── valence_generated/     # 代码生成（方块/物品元数据）✅
├── valence_anvil/         # Anvil 世界格式 ✅
├── valence_nbt/           # NBT 编解码 ✅
├── valence_vanilla/       # 原版游戏机制 ⚠️ 重点评审区域
├── valence_world/         # 世界管理/保存 ⚠️ 未完全实现
├── valence_network/       # 网络连接层 ✅
└── ... 其他辅助 crate
```

**评价**：Crate 边界划分基本遵循 Valence 原架构，**valence_vanilla** 作为游戏机制聚合层是合理的。但缺少一个关键的 **调度器/线程模型 crate**——这是实现真正的区域化（Folia 式）多线程的核心。

---

## 二、核心架构评审

### 2.1 ECS 架构与系统调度（⚠️ 中等风险）

#### 现状

所有原版机制系统都注册在 Bevy 的 `Update` 阶段：

```rust
// valence_vanilla/src/lib.rs
app.add_plugins((
    block_update::BlockUpdatePlugin,
    tick_schedule::TickSchedulePlugin,
    hopper::HopperPlugin,
    crop::CropPlugin,
    redstone::RedstonePlugin,
    villager::VillagerPlugin,
    physics::PhysicsPlugin,
    entity_ai::EntityAiPlugin,
    combat::CombatPlugin,
    mob_spawning::MobSpawningPlugin,
    terrain::TerrainPlugin,
    crafting::CraftingPlugin,
));
```

每个 Plugin 内部再细分 `.chain()` 的系统组，例如物理：

```rust
app.add_systems(Update, (
    apply_gravity,
    apply_drag,
    integrate_motion,
    solve_collisions,
).chain());
```

#### 问题

1. **缺少 SystemSet 显式排序**：不同 Plugin 之间的系统没有明确的相对顺序。例如红石系统和方块更新系统谁先谁后？物理和 AI 谁先？这会导致不确定的 tick 行为。
2. **没有阶段化（Phased）调度**：原版 Minecraft 的 tick 是严格分阶段的——实体移动 → 实体碰撞 → 方块随机 tick → 方块计划 tick → 红石更新 → ……当前所有系统挤在 `Update` 一个阶段，无法保证原版语义。
3. **缺少固定时间步长（Fixed Timestep）**：`physics.rs` 使用了 `time.delta_seconds()`，这意味着物理模拟帧率相关。对于 Minecraft 20 TPS 的固定 tick 节奏，应该用固定步长而非可变步长。

#### 建议

```rust
// 建议引入 VanillaTick 阶段
#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum VanillaTickSet {
    PreTick,        // 准备工作
    EntityAction,   // 实体行为决策
    EntityMovement, // 实体移动
    Physics,        // 碰撞解析
    BlockRandomTick,// 随机 tick
    BlockScheduled, // 计划 tick
    Redstone,       // 红石更新
    BlockUpdate,    // 方块状态传播
    PostTick,       // 清理、脏标记
}

// 注册到 FixedUpdate（20 TPS）
app.configure_sets(FixedUpdate, (
    VanillaTickSet::PreTick,
    VanillaTickSet::EntityAction.after(VanillaTickSet::PreTick),
    VanillaTickSet::EntityMovement.after(VanillaTickSet::EntityAction),
    // ... 以此类推
));
```

---

### 2.2 区域化/多线程模型（🔴 高风险——尚未实现）

#### 现状

当前架构**完全没有区域化（Regionized）设计**：

- 所有系统使用 `Query<&mut ChunkLayer>` 遍历**所有** Layer
- 红石更新在**单个系统**中串行处理所有待更新方块
- 物理碰撞检测使用 `chunk_layers.get_single()`，假设只有一个世界
- 没有 Region、Chunk Section 或独立调度单元的概念

#### 与用户目标的差距

用户的目标是实现 **Folia 式的区域化多线程核心**，即：
- 世界按 Region（32×32 区块）或更细粒度划分独立调度单元
- 每个 Region 的 tick 可以在独立线程上并行执行
- Region 之间通过消息传递同步（如实体跨 Region 移动）

当前代码距离这一目标有显著差距。

#### 建议路线

**短期（当前基线）**：
在 Bevy ECS 内先实现正确的**单线程 tick 顺序**，作为功能基线。

**中期**：
引入 **Region Resource** 模式：

```rust
#[derive(Resource)]
pub struct RegionManager {
    regions: HashMap<RegionPos, Entity>, // 每个 Region 是一个 ECS Entity
}

// Region 组件
#[derive(Component)]
pub struct Region {
    pub pos: RegionPos,
    pub chunks: HashMap<ChunkPos, Entity>,
    pub tick_pending: bool,
}

// 系统按 Region 并行执行
app.add_systems(FixedUpdate, tick_region.redstone.run_if(region_tick_pending));
```

利用 Bevy ECS 的 `QueryParIter` 或自定义 `Schedule` 实现 Region 级并行。

**长期**：
如果 Valence 的 ECS 模型成为性能瓶颈，考虑在 Region 内部署独立的 `bevy::app::App` 实例，通过 channel 跨 Region 通信。

---

### 2.3 红石系统（⚠️ 中等风险）

#### 现状

`valence_vanilla/src/redstone/mod.rs` 实现了一套基于更新队列的信号传播系统：

- `RedstoneUpdateQueue` 存储待更新的方块位置
- `update_redstone_components` 每帧处理队列中的所有条目
- 各元件（Wire/Torch/Repeater/Comparator/Piston/Lamp）有独立的处理函数

#### 优点

- 使用队列驱动更新，避免递归爆炸
- 元件逻辑分离到独立模块，可维护性好

#### 问题

1. **单帧无限循环风险**：

```rust
fn update_redstone_components(...) {
    let positions: Vec<BlockPos> = update_queue.iter().map(|e| e.pos).collect();
    update_queue.clear(); // 清空队列
    
    for mut chunk_layer in &mut chunk_layers {
        for pos in positions.iter().copied() {
            // ... 处理方块，可能又会 push 到 update_queue
        }
    }
    // 本帧不会处理新 push 的条目！它们要等到下一帧
}
```

   实际上代码先 `collect()` 再 `clear()`，所以本帧新 push 的条目确实被推迟到了下一帧。这避免了无限循环，但会导致**大型红石电路需要多帧才能稳定**——与原版单 tick 内稳定的行为不一致。

2. **信号传播不是原版顺序（BUD/零 tick 问题）**：
   原版红石更新有严格的顺序（PP update order），当前实现没有模拟这一点，可能导致 BUD（Block Update Detector）等行为与原版不一致。

3. **ChunkLayer 遍历开销**：
   `for mut chunk_layer in &mut chunk_layers` + `get_block_state` 每次都会遍历所有 Layer。如果只有一个世界，应该直接获取目标 Layer，避免 Query 遍历。

#### 建议

1. **引入迭代稳定机制**：

```rust
fn update_redstone_components(..., mut queue: ResMut<RedstoneUpdateQueue>) {
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 1000;
    
    while !queue.is_empty() && iterations < MAX_ITERATIONS {
        iterations += 1;
        let batch: Vec<_> = queue.drain(..).collect();
        for entry in batch {
            process_redstone_entry(entry, ...);
        }
    }
}
```

2. **长期考虑 BUD/更新顺序**：引入方块位置哈希来决定更新顺序，近似原版行为。

---

### 2.4 Tick 调度器（⚠️ 中等风险）

#### 现状

`tick_schedule.rs` 实现了：
- `TickScheduler` Resource，维护 `current_tick: u64`
- 计划 tick 存储在 `Vec<ScheduledTick>` 中
- 随机 tick 按区块/section 生成事件

#### 问题

1. **计划 tick 数据结构效率低**：

```rust
fn drain_due_ticks(&mut self) -> Vec<ScheduledTick> {
    self.scheduled_ticks.retain(|t| { ... }); // O(n) 线性扫描
}
```

   每帧对全量计划 tick 做 `retain` 是 O(n)。对于大型服务器，计划 tick 可能成千上万（水流、熔岩、作物），这里会成为瓶颈。

2. **没有按 Chunk/Region 分区**：所有计划 tick 存在一个全局 Vec 中，无法利用局部性。

3. **随机 tick 遍历所有 section**：`for (chunk_pos, _) in layer.chunks()` 遍历每个已加载区块的每个 section，再生成 `speed` 个随机坐标。对于大视距，这是 O(加载区块数 × section数 × speed)。

#### 建议

1. **计划 tick 改用 BinaryHeap 或 BTreeMap**：

```rust
#[derive(Resource)]
pub struct TickScheduler {
    scheduled_ticks: BinaryHeap<ScheduledTickEntry>, // 按 tick 时间排序
}

struct ScheduledTickEntry {
    tick: u64,
    pos: BlockPos,
    // ...
}

impl Ord for ScheduledTickEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.tick.cmp(&self.tick) // 最小堆
    }
}
```

   这样 `drain_due_ticks` 变成 O(k log n)，k 为到期的 tick 数。

2. **按 Region/Chunk 分桶存储计划 tick**，减少每帧扫描范围。

---

### 2.5 物理引擎（⚠️ 中等风险）

#### 现状

`physics.rs` 实现了：
- 重力、阻力、速度积分（欧拉法）
- AABB 与方块的碰撞检测和穿透解析

#### 优点

- 系统分离清晰：`apply_gravity` → `apply_drag` → `integrate_motion` → `solve_collisions`，用 `.chain()` 保证顺序
- 参数化设计（gravity_multiplier、drag、mass）

#### 问题

1. **使用可变时间步长**：

```rust
fn integrate_motion(..., time: Res<Time>) {
    let dt = time.delta_seconds() as f64;
    body.velocity += accel * dt;
    position.0 += body.velocity * dt;
}
```

   Minecraft 原版物理是离散 tick（固定 50ms），使用可变 `dt` 会导致不同帧率下物理行为不一致。应改用 `dt = 1.0 / 20.0`（固定 tick）。

2. **碰撞解析效率低**：

```rust
for x in min_block.x..=max_block.x {
    for y in min_block.y..=max_block.y {
        for z in min_block.z..=max_block.z {
            // 遍历 AABB 内的所有方块
        }
    }
}
```

   三层嵌套循环，且对每个实体、每帧都执行。当实体密度高时，这是 O(实体数 × AABB体积)。原版 Minecraft 使用更粗略的碰撞检测优化。

3. **没有实体-实体碰撞**：`collides_with_entities` 字段存在但没有任何系统处理实体间碰撞。

4. **只支持单个 ChunkLayer**：`chunk_layers.get_single()` 会 panic 如果有多个世界。

#### 建议

1. 物理系统迁移到 `FixedUpdate`，使用 `dt = 1.0 / 20.0`。
2. 碰撞检测引入**空间哈希**或复用 ChunkLayer 的已有空间结构，避免全量扫描。
3. 为每个实体关联其所在的 `ChunkLayer` Entity，避免全局 Query。

---

### 2.6 实体 AI 与寻路（🔴 高风险——核心功能缺失）

#### 现状

`entity_ai/` 目录包含：
- `behavior.rs`：行为树（Sequence、Selector、Condition）
- `memory.rs`：实体记忆
- `perception.rs`：感知系统
- `pathfinding.rs`：寻路

但 `pathfinding.rs` 的实际实现是：

```rust
pub fn find_path(ctx: &PathfindingContext) -> PathfindingResult {
    // 计算起点到终点的直线距离
    // 线性插值生成路径点
    // 完全不考虑方块是否可通行！
}
```

这是**一个没有任何实际寻路功能的占位实现**。生物会直线穿墙移动。

#### 建议

`find_path` 需要真正的 A* 实现：

```rust
pub fn find_path(ctx: &PathfindingContext, layer: &ChunkLayer) -> PathfindingResult {
    let mut open_set = BinaryHeap::new();
    let mut came_from = HashMap::new();
    let mut g_score = HashMap::new();
    
    g_score.insert(ctx.start, 0);
    open_set.push(Node { pos: ctx.start, f_score: heuristic(ctx.start, ctx.goal) });
    
    while let Some(current) = open_set.pop() {
        if current.pos == ctx.goal { return reconstruct_path(&came_from, ctx.goal); }
        
        for neighbor in walkable_neighbors(current.pos, layer) {
            let tentative_g = g_score[&current.pos] + 1;
            if tentative_g < *g_score.get(&neighbor).unwrap_or(&u32::MAX) {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                open_set.push(Node { pos: neighbor, f_score: tentative_g + heuristic(neighbor, ctx.goal) });
            }
        }
    }
    PathfindingResult::Failed
}
```

同时需要定义 `walkable_neighbors` 来检查方块碰撞箱和高度。

---

### 2.7 世界保存系统（🔴 高风险——未完全实现）

#### 现状

`valence_world/src/save_system.rs` 实现了管理框架：
- `WorldSaveManager` Resource
- 脏区块追踪
- 自动保存定时器（5 分钟）
- `level.dat` 读写

#### 问题

`perform_save` 函数**没有实际保存区块数据**：

```rust
fn perform_save(save_manager: &mut WorldSaveManager, _layers: &Query<&ChunkLayer>) 
    -> Result<(), Box<dyn std::error::Error>> 
{
    // ... 创建目录、保存 level.dat ...
    
    let dirty: Vec<_> = save_manager.dirty_chunks().iter().copied().collect();
    for (chunk_x, chunk_z) in &dirty {
        info!("Saving chunk ({}, {})", chunk_x, chunk_z); // 只打印日志！
    }
    
    save_manager.clear_dirty();
    Ok(()) // 区块数据并没有写入 .mca 文件
}
```

这是一个**骨架实现**，核心功能缺失。

#### 建议

1. 集成 `valence_anvil` crate 的 `AnvilFolder` / `RegionFolder` API 将 `ChunkLayer` 序列化为 Anvil 格式。
2. 保存应异步化，避免阻塞主 tick 线程：

```rust
fn auto_save_system(
    mut save_manager: ResMut<WorldSaveManager>,
    layers: Query<&ChunkLayer>,
    mut save_tasks: ResMut<SaveTaskPool>, // 保存任务池
) {
    if save_manager.should_auto_save() {
        for layer in &layers {
            let chunks = collect_dirty_chunks(layer, &save_manager);
            save_tasks.spawn(async move {
                save_chunks_to_anvil(chunks).await;
            });
        }
    }
}
```

---

### 2.8 方块更新传播（✅ 良好）

`block_update.rs` 的设计值得肯定：

- `BlockUpdateEvent` + `NeighborUpdateEvent` 双事件模型清晰
- `set_block_with_update` 封装了"设置方块+通知邻居"的完整语义
- `set_block_with_delayed_update` 支持延迟更新（用于活塞等）

** minor 建议**：`NeighborUpdateEvent` 的消耗系统 `process_neighbor_updates` 目前是空消耗，应确保各游戏机制（红石、活塞、水）确实在监听此事件，否则这些是性能开销。

---

## 三、关键缺陷汇总

| 优先级 | 模块 | 问题 | 影响 |
|--------|------|------|------|
| 🔴 P0 | AI/寻路 | `find_path` 是线性插值占位实现，生物穿墙 | 游戏无法玩 |
| 🔴 P0 | 世界保存 | `perform_save` 未实际写入区块数据 | 进度丢失 |
| 🔴 P0 | 区域化 | 无 Region/独立调度单元，与 Folia 目标不符 | 无法水平扩展 |
| ⚠️ P1 | 物理 | 可变 `dt`，碰撞检测 O(n³) | 帧率不稳、卡顿 |
| ⚠️ P1 | Tick调度 | `Vec` 存储计划 tick，O(n) drain | 大服性能瓶颈 |
| ⚠️ P1 | 红石 | 多帧稳定，无 BUD 顺序 | 红石行为与原版不一致 |
| ⚠️ P1 | 系统调度 | 所有系统挤在 `Update`，无阶段化 | 不确定行为 |
| 💡 P2 | 网络/协议 | — | 当前实现良好，无显著问题 |

---

## 四、优先修复建议

### 第一阶段：功能基线（单线程正确性）

1. **实现真正的 A* 寻路**（`entity_ai/pathfinding.rs`）
2. **完成世界保存**（`save_system.rs` 接入 `valence_anvil`）
3. **固定时间步长**（物理迁移到 `FixedUpdate`，`dt = 0.05`）
4. **tick 阶段化**（定义 `VanillaTickSet`，明确系统顺序）

### 第二阶段：性能与扩展性

5. **计划 tick 改用 BinaryHeap**
6. **红石迭代稳定**（单 tick 内迭代至稳定或上限）
7. **碰撞检测优化**（空间分区）
8. **多 ChunkLayer 支持**（物理、红石不再 `get_single()`）

### 第三阶段：区域化（Folia 式并行）

9. **设计 Region 抽象**（Region Entity + RegionManager）
10. **Region 内并行 tick**（利用 `bevy_ecs` 的并行查询）
11. **跨 Region 实体迁移**（消息传递/命令队列）
12. **独立 Region 保存**（支持 Region 级异步 I/O）

---

## 五、代码质量亮点

- **模块化 Plugin 架构**：每个机制独立成 Plugin，易于扩展和测试
- **事件驱动设计**：`BlockUpdateEvent`、`RandomTickEvent` 等事件解耦了系统
- **Clippy 严格**：大量 clippy lint 启用，代码风格统一
- **测试覆盖**：`save_system.rs` 包含单元测试，值得保持

---

## 六、与 Folia 架构的差距

| 能力 | Folia (Paper) | Mili-rust 当前 | 差距 |
|------|---------------|----------------|------|
| 区域划分 | Region（32×32 区块）独立 | 无 | 🔴 大 |
| 线程模型 | 每 Region 独立线程调度 | 单线程 ECS | 🔴 大 |
| 实体迁移 | 跨 Region 实体转移 | 无 | 🔴 大 |
| 全局实体 | 命令方块、计分板等全局同步 | 无全局概念 | ⚠️ 中等 |
| 网络 I/O | 异步 Netty | tokio + 自定义 | ✅ 接近 |
| ECS 并行 | 无（纯命令式） | Bevy ECS 部分并行 | ✅ 有优势 |

**结论**：当前项目是一个基于 Valence 的功能扩展服，距离真正的区域化核心尚有架构级差距。但 Bevy ECS 的数据驱动模型反而为后续实现 Region 并行提供了比 Folia 更好的基础——只需在 ECS 层面将 World 按 Region 拆分即可。

---

*报告结束。如需针对某一模块深入评审或提供具体实现代码，请指明。*
