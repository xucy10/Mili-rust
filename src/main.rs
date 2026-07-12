use bevy_time::TimePlugin;
use valence::prelude::*;
use valence_vanilla::block_update::{BlockUpdateEvent, NeighborUpdateEvent};
use valence_vanilla::VanillaPlugin;
use valence_world::save_system::{WorldSaveManager, WorldSavePlugin};

const SPAWN_Y: i32 = 64;
const SERVER_PORT: u16 = 25565;

pub fn main() {
    println!("========================================");
    println!("    Mili-rust Minecraft Server");
    println!("    版本: 1.20.1");
    println!("    端口: {SERVER_PORT}");
    println!("========================================");
    println!();
    println!("启动服务器...");
    println!("用 Minecraft 1.20.1 连接 localhost:{SERVER_PORT}");
    println!("按 Ctrl+C 停止服务器");
    println!();

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TimePlugin)
        .add_plugins(VanillaPlugin)
        .add_plugins(WorldSavePlugin)
        .insert_resource(ServerPort(SERVER_PORT))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                init_clients,
                despawn_disconnected_clients,
                handle_block_updates,
                handle_neighbor_updates,
            ),
        )
        .run();
}

#[derive(Resource)]
#[allow(dead_code)]
struct ServerPort(u16);

fn setup(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
) {
    println!("生成世界...");

    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    // 创建区块
    for z in -5..5 {
        for x in -5..5 {
            layer.chunk.insert_chunk([x, z], UnloadedChunk::new());
        }
    }

    // 生成基础地形
    for z in -50..50 {
        for x in -50..50 {
            // 基础地面
            layer
                .chunk
                .set_block([x, SPAWN_Y, z], BlockState::GRASS_BLOCK);
            layer.chunk.set_block([x, SPAWN_Y - 1, z], BlockState::DIRT);
            layer
                .chunk
                .set_block([x, SPAWN_Y - 2, z], BlockState::STONE);

            // 简单的山丘
            let height = ((f64::from(x) * 0.1).sin() * (f64::from(z) * 0.1).cos() * 5.0) as i32;
            for y in SPAWN_Y + 1..=SPAWN_Y + height {
                layer.chunk.set_block([x, y, z], BlockState::STONE);
            }
        }
    }

    // 红石演示区域
    println!("创建红石演示区域...");
    create_redstone_demo(&mut layer);

    // 农场演示区域
    println!("创建农场演示区域...");
    create_farm_demo(&mut layer);

    // 漏斗演示区域
    println!("创建漏斗演示区域...");
    create_hopper_demo(&mut layer);

    commands.spawn(layer);

    println!("世界生成完成!");
    println!();

    // 初始化世界保存管理器
    commands.insert_resource(WorldSaveManager::new("./world"));
}

fn create_redstone_demo(layer: &mut LayerBundle) {
    let base_x = -5;
    let base_z = -5;

    // 红石线
    for x in 0..10 {
        layer
            .chunk
            .set_block([base_x + x, SPAWN_Y + 1, base_z], BlockState::REDSTONE_WIRE);
    }

    // 红石火把（信号源）
    layer.chunk.set_block(
        [base_x - 1, SPAWN_Y + 1, base_z],
        BlockState::REDSTONE_TORCH,
    );

    // 红石灯（显示信号）
    layer.chunk.set_block(
        [base_x + 10, SPAWN_Y + 1, base_z],
        BlockState::REDSTONE_LAMP,
    );

    // 中继器
    layer
        .chunk
        .set_block([base_x + 5, SPAWN_Y + 1, base_z + 1], BlockState::REPEATER);

    // 比较器
    layer.chunk.set_block(
        [base_x + 5, SPAWN_Y + 1, base_z - 1],
        BlockState::COMPARATOR,
    );

    // 活塞
    layer
        .chunk
        .set_block([base_x + 3, SPAWN_Y + 1, base_z + 2], BlockState::PISTON);
}

fn create_farm_demo(layer: &mut LayerBundle) {
    let base_x = 5;
    let base_z = -5;

    // 耕地
    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, SPAWN_Y, base_z], BlockState::FARMLAND);

        // 水源
        if x == 2 {
            layer
                .chunk
                .set_block([base_x + x, SPAWN_Y, base_z - 1], BlockState::WATER);
        }
    }

    // 作物
    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, SPAWN_Y + 1, base_z], BlockState::WHEAT);
    }

    // 胡萝卜
    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, SPAWN_Y, base_z + 2], BlockState::FARMLAND);
        layer
            .chunk
            .set_block([base_x + x, SPAWN_Y + 1, base_z + 2], BlockState::CARROTS);
    }
}

fn create_hopper_demo(layer: &mut LayerBundle) {
    let base_x = -5;
    let base_z = 5;

    // 箱子
    layer
        .chunk
        .set_block([base_x, SPAWN_Y, base_z], BlockState::CHEST);

    // 漏斗连接到箱子
    layer
        .chunk
        .set_block([base_x, SPAWN_Y + 1, base_z], BlockState::HOPPER);

    // 另一个漏斗
    layer
        .chunk
        .set_block([base_x + 1, SPAWN_Y + 1, base_z], BlockState::HOPPER);

    // 熔炉
    layer
        .chunk
        .set_block([base_x + 1, SPAWN_Y, base_z], BlockState::FURNACE);
}

fn init_clients(
    mut clients: Query<
        (
            &mut Client,
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut Inventory,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
) {
    for (
        mut _client,
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut pos,
        mut inventory,
    ) in &mut clients
    {
        let layer = layers.single();

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        visible_entity_layers.0.insert(layer);
        pos.set([0.0, f64::from(SPAWN_Y + 2), 0.0]);

        // 给玩家一些初始物品
        inventory.set_slot(36, ItemStack::new(ItemKind::DiamondPickaxe, 1, None));
        inventory.set_slot(37, ItemStack::new(ItemKind::DiamondShovel, 1, None));
        inventory.set_slot(38, ItemStack::new(ItemKind::DiamondAxe, 1, None));
        inventory.set_slot(39, ItemStack::new(ItemKind::Cobblestone, 64, None));
        inventory.set_slot(40, ItemStack::new(ItemKind::RedstoneTorch, 16, None));
        inventory.set_slot(41, ItemStack::new(ItemKind::Redstone, 16, None));
        inventory.set_slot(42, ItemStack::new(ItemKind::Repeater, 4, None));
        inventory.set_slot(43, ItemStack::new(ItemKind::Comparator, 4, None));
        inventory.set_slot(44, ItemStack::new(ItemKind::RedstoneLamp, 4, None));

        println!("新玩家加入游戏!");
    }
}

fn handle_block_updates(mut events: EventReader<BlockUpdateEvent>) {
    for event in events.read() {
        println!(
            "[方块更新] 位置: ({}, {}, {}) 类型: {:?}",
            event.position.x, event.position.y, event.position.z, event.new_state
        );
    }
}

fn handle_neighbor_updates(mut events: EventReader<NeighborUpdateEvent>) {
    for _event in events.read() {
        // 邻居更新日志太多，这里不打印
    }
}
