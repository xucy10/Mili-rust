mod callbacks;
mod config;

use bevy_time::TimePlugin;
use valence::prelude::*;
use valence_server::{CompressionThreshold, ServerSettings};
use valence_vanilla::block_update::{BlockUpdateEvent, NeighborUpdateEvent};
use valence_vanilla::crafting::{register_vanilla_recipes, CraftingRegistry};
use valence_vanilla::mob_spawning::{spawn_mob, MobType};
use valence_vanilla::terrain::TerrainSeed;
use valence_vanilla::VanillaPlugin;
use valence_world::save_system::{WorldSaveManager, WorldSavePlugin};
use valence_world::LevelDat;

use crate::callbacks::MiliCallbacks;
use crate::config::ServerConfig;

pub fn main() {
    #[cfg(windows)]
    unsafe {
        extern "system" {
            fn AllocConsole() -> i32;
            fn SetConsoleOutputCP(codepage: u32) -> i32;
            fn SetConsoleCP(codepage: u32) -> i32;
        }
        AllocConsole();
        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);
    }

    let config_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
        .unwrap_or_else(|| "config.toml".into());

    let config = ServerConfig::load_or_create(&config_path);

    let port = config.port;
    let online = config.online_mode;

    println!("========================================");
    println!("    Mili-rust Minecraft Server");
    println!("    端口: {port}");
    println!("    正版验证: {}", if online { "开启" } else { "关闭" });
    println!(
        "    压缩: {}",
        if config.compression_enabled {
            format!("开启 (threshold={})", config.network_compression_threshold)
        } else {
            "关闭".into()
        }
    );
    if !config.motd.is_empty() {
        println!("    MOTD: {}", config.motd[0]);
    }
    println!("========================================");
    println!();
    println!("启动服务器...");
    println!("用 Minecraft 连接 localhost:{port}");
    println!("按 Ctrl+C 停止服务器");
    println!();

    let connection_mode = config.connection_mode();
    let address = format!("0.0.0.0:{port}").parse().unwrap();

    let network_settings = NetworkSettings {
        address,
        connection_mode,
        callbacks: MiliCallbacks::from(&config).into(),
        ..Default::default()
    };

    let compression_threshold = if config.compression_enabled {
        CompressionThreshold(config.network_compression_threshold)
    } else {
        CompressionThreshold(-1)
    };
    let server_settings = ServerSettings {
        compression_threshold,
        ..Default::default()
    };

    App::new()
        .insert_resource(server_settings)
        .insert_resource(network_settings)
        .add_plugins(DefaultPlugins)
        .add_plugins(TimePlugin)
        .add_plugins(VanillaPlugin)
        .add_plugins(WorldSavePlugin)
        .insert_resource(config)
        .add_systems(PreStartup, register_crafting_recipes)
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

fn register_crafting_recipes(mut registry: ResMut<CraftingRegistry>) {
    register_vanilla_recipes(&mut registry);
    println!("已注册合成配方");
}

fn setup(
    mut commands: Commands,
    server: Res<Server>,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    config: Res<ServerConfig>,
    terrain_seed: Res<TerrainSeed>,
) {
    let chunk_radius = config.chunk_render_distance as i32;
    let seed = terrain_seed.seed;

    println!("生成主世界 (种子: {seed})...");
    let mut overworld = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);

    let gen = valence_vanilla::terrain::TerrainGenerator::new(
        valence_vanilla::terrain::Dimension::Overworld,
        seed,
    );

    for z in -chunk_radius..chunk_radius {
        for x in -chunk_radius..chunk_radius {
            let chunk_data = gen.generate_chunk(x, z);
            overworld.chunk.insert_chunk([x, z], chunk_data);
        }
    }

    create_spawn_area(&mut overworld, config.spawn.y);
    commands.spawn(overworld);
    println!("主世界生成完成!");

    println!("生成下界...");
    let mut nether = LayerBundle::new(ident!("the_nether"), &dimensions, &biomes, &server);

    let nether_gen = valence_vanilla::terrain::TerrainGenerator::new(
        valence_vanilla::terrain::Dimension::Nether,
        seed.wrapping_add(1000),
    );

    let nether_radius = (chunk_radius / 2).max(3);
    for z in -nether_radius..nether_radius {
        for x in -nether_radius..nether_radius {
            let chunk_data = nether_gen.generate_chunk(x, z);
            nether.chunk.insert_chunk([x, z], chunk_data);
        }
    }

    commands.spawn(nether);
    println!("下界生成完成!");

    println!("生成末地...");
    let mut end = LayerBundle::new(ident!("the_end"), &dimensions, &biomes, &server);

    let end_gen = valence_vanilla::terrain::TerrainGenerator::new(
        valence_vanilla::terrain::Dimension::End,
        seed.wrapping_add(2000),
    );

    let end_radius = (chunk_radius / 2).max(3);
    for z in -end_radius..end_radius {
        for x in -end_radius..end_radius {
            let chunk_data = end_gen.generate_chunk(x, z);
            end.chunk.insert_chunk([x, z], chunk_data);
        }
    }

    commands.spawn(end);
    println!("末地生成完成!");

    println!("生成初始村民...");
    let spawn_y = config.spawn.y;
    let villager_positions = [
        DVec3::new(8.0, spawn_y as f64 + 2.0, 8.0),
        DVec3::new(-8.0, spawn_y as f64 + 2.0, 8.0),
        DVec3::new(8.0, spawn_y as f64 + 2.0, -8.0),
    ];

    for pos in villager_positions {
        spawn_mob(&mut commands, MobType::Villager, pos);
    }

    println!("世界生成完成!");
    println!();

    let mut save_manager = WorldSaveManager::new("./world");
    let mut level_dat = LevelDat::with_name("Mili-rust World");
    level_dat.set_seed(seed as i64);
    level_dat.level_data.spawn_x = config.spawn.x;
    level_dat.level_data.spawn_y = config.spawn.y;
    level_dat.level_data.spawn_z = config.spawn.z;
    let _ = save_manager.load_level_dat();
    save_manager.level_dat = Some(level_dat);
    let _ = save_manager.save_level_dat();
    commands.insert_resource(save_manager);
}

fn create_spawn_area(layer: &mut LayerBundle, spawn_y: i32) {
    for x in -2..=2 {
        for z in -2..=2 {
            layer.chunk.set_block([x, spawn_y + 1, z], BlockState::AIR);
        }
    }

    layer
        .chunk
        .set_block([0, spawn_y + 1, 0], BlockState::CRAFTING_TABLE);

    create_redstone_demo(layer, spawn_y);
    create_farm_demo(layer, spawn_y);
    create_hopper_demo(layer, spawn_y);
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
    let spawn_y = config.spawn.y;

    for (
        mut layer_id,
        mut visible_chunk_layer,
        mut visible_entity_layers,
        mut pos,
        mut inventory,
    ) in &mut clients
    {
        let layer = layers.iter().next().unwrap_or_else(|| layers.single());

        layer_id.0 = layer;
        visible_chunk_layer.0 = layer;
        for l in &layers {
            visible_entity_layers.0.insert(l);
        }
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
        inventory.set_slot(9, ItemStack::new(ItemKind::OakPlanks, 64, None));
        inventory.set_slot(10, ItemStack::new(ItemKind::IronIngot, 32, None));
        inventory.set_slot(11, ItemStack::new(ItemKind::Coal, 32, None));
        inventory.set_slot(12, ItemStack::new(ItemKind::CraftingTable, 4, None));
        inventory.set_slot(13, ItemStack::new(ItemKind::Furnace, 2, None));

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
    for _event in events.read() {}
}