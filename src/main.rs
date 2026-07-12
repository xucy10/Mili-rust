mod config;

use bevy_time::TimePlugin;
use valence::prelude::*;
use valence_vanilla::block_update::{BlockUpdateEvent, NeighborUpdateEvent};
use valence_vanilla::VanillaPlugin;
use valence_world::save_system::{WorldSaveManager, WorldSavePlugin};

use crate::config::ServerConfig;

pub fn main() {
    // Windows: 双击exe时分配控制台窗口
    #[cfg(windows)]
    unsafe {
        extern "system" {
            fn AllocConsole() -> i32;
            fn SetConsoleOutputCP(codepage: u32) -> i32;
            fn SetConsoleCP(codepage: u32) -> i32;
        }
        // AllocConsole 在已有控制台时不会失败
        AllocConsole();
        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);
    }

    // 配置文件路径: exe所在目录/config.toml
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
        .unwrap_or_else(|| "config.toml".into());

    let config = ServerConfig::load_or_create(&config_path);

    let port = config.server.port;
    let online = config.server.online_mode;

    println!("========================================");
    println!("    Mili-rust Minecraft Server");
    println!("    端口: {port}");
    println!("    正版验证: {}", if online { "开启" } else { "关闭" });
    println!("========================================");
    println!();
    println!("启动服务器...");
    println!("用 Minecraft 连接 localhost:{port}");
    println!("按 Ctrl+C 停止服务器");
    println!();

    let connection_mode = config.connection_mode();
    let address = format!("0.0.0.0:{port}").parse().unwrap();

    // NetworkSettings 必须在 DefaultPlugins 之前插入，
    // 因为 NetworkPlugin 用 get_resource_or_insert_with 读取，
    // 如果 DefaultPlugins 先构建会覆盖我们的设置
    let network_settings = NetworkSettings {
        address,
        connection_mode,
        ..Default::default()
    };

    App::new()
        .insert_resource(network_settings)
        .add_plugins(DefaultPlugins)
        .add_plugins(TimePlugin)
        .add_plugins(VanillaPlugin)
        .add_plugins(WorldSavePlugin)
        .insert_resource(config)
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

fn setup(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    config: Res<ServerConfig>,
) {
    let spawn_y = config.world.spawn_y;
    let terrain_radius = config.world.terrain_radius;
    let chunk_radius = config.world.chunk_radius;

    println!("生成世界...");

    let mut layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    for z in -chunk_radius..chunk_radius {
        for x in -chunk_radius..chunk_radius {
            layer.chunk.insert_chunk([x, z], UnloadedChunk::new());
        }
    }

    for z in -terrain_radius..terrain_radius {
        for x in -terrain_radius..terrain_radius {
            layer
                .chunk
                .set_block([x, spawn_y, z], BlockState::GRASS_BLOCK);
            layer.chunk.set_block([x, spawn_y - 1, z], BlockState::DIRT);
            layer
                .chunk
                .set_block([x, spawn_y - 2, z], BlockState::STONE);

            let height = ((f64::from(x) * 0.1).sin() * (f64::from(z) * 0.1).cos() * 5.0) as i32;
            for y in spawn_y + 1..=spawn_y + height {
                layer.chunk.set_block([x, y, z], BlockState::STONE);
            }
        }
    }

    println!("创建红石演示区域...");
    create_redstone_demo(&mut layer, spawn_y);

    println!("创建农场演示区域...");
    create_farm_demo(&mut layer, spawn_y);

    println!("创建漏斗演示区域...");
    create_hopper_demo(&mut layer, spawn_y);

    commands.spawn(layer);

    println!("世界生成完成!");
    println!();

    commands.insert_resource(WorldSaveManager::new("./world"));
}

fn create_redstone_demo(layer: &mut LayerBundle, spawn_y: i32) {
    let base_x = -5;
    let base_z = -5;

    for x in 0..10 {
        layer
            .chunk
            .set_block([base_x + x, spawn_y + 1, base_z], BlockState::REDSTONE_WIRE);
    }

    layer.chunk.set_block(
        [base_x - 1, spawn_y + 1, base_z],
        BlockState::REDSTONE_TORCH,
    );

    layer.chunk.set_block(
        [base_x + 10, spawn_y + 1, base_z],
        BlockState::REDSTONE_LAMP,
    );

    layer
        .chunk
        .set_block([base_x + 5, spawn_y + 1, base_z + 1], BlockState::REPEATER);

    layer.chunk.set_block(
        [base_x + 5, spawn_y + 1, base_z - 1],
        BlockState::COMPARATOR,
    );

    layer
        .chunk
        .set_block([base_x + 3, spawn_y + 1, base_z + 2], BlockState::PISTON);
}

fn create_farm_demo(layer: &mut LayerBundle, spawn_y: i32) {
    let base_x = 5;
    let base_z = -5;

    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, spawn_y, base_z], BlockState::FARMLAND);

        if x == 2 {
            layer
                .chunk
                .set_block([base_x + x, spawn_y, base_z - 1], BlockState::WATER);
        }
    }

    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, spawn_y + 1, base_z], BlockState::WHEAT);
    }

    for x in 0..5 {
        layer
            .chunk
            .set_block([base_x + x, spawn_y, base_z + 2], BlockState::FARMLAND);
        layer
            .chunk
            .set_block([base_x + x, spawn_y + 1, base_z + 2], BlockState::CARROTS);
    }
}

fn create_hopper_demo(layer: &mut LayerBundle, spawn_y: i32) {
    let base_x = -5;
    let base_z = 5;

    layer
        .chunk
        .set_block([base_x, spawn_y, base_z], BlockState::CHEST);

    layer
        .chunk
        .set_block([base_x, spawn_y + 1, base_z], BlockState::HOPPER);

    layer
        .chunk
        .set_block([base_x + 1, spawn_y + 1, base_z], BlockState::HOPPER);

    layer
        .chunk
        .set_block([base_x + 1, spawn_y, base_z], BlockState::FURNACE);
}

fn init_clients(
    mut clients: Query<
        (
            &mut EntityLayerId,
            &mut VisibleChunkLayer,
            &mut VisibleEntityLayers,
            &mut Position,
            &mut Inventory,
        ),
        Added<Client>,
    >,
    layers: Query<Entity, (With<ChunkLayer>, With<EntityLayer>)>,
    config: Res<ServerConfig>,
) {
    let spawn_y = config.world.spawn_y;

    for (
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
        pos.set([0.0, f64::from(spawn_y + 2), 0.0]);

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
