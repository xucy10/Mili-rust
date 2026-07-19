use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use noise::{Fbm, NoiseFn, Perlin, SuperSimplex};
use rand::Rng;
use valence_generated::block::BlockState;
use valence_protocol::{BlockPos, Ident};
use valence_server::layer::chunk::{Chunk, ChunkLayer, UnloadedChunk};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainSeed>();
    }
}

#[derive(Resource)]
pub struct TerrainSeed {
    pub seed: u32,
}

impl Default for TerrainSeed {
    fn default() -> Self {
        Self {
            seed: rand::thread_rng().gen(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}

impl Dimension {
    pub fn dimension_type_name(&self) -> Ident<String> {
        match self {
            Dimension::Overworld => Ident::new("minecraft:overworld").unwrap().to_string_ident(),
            Dimension::Nether => Ident::new("minecraft:the_nether").unwrap().to_string_ident(),
            Dimension::End => Ident::new("minecraft:the_end").unwrap().to_string_ident(),
        }
    }

    pub fn sea_level(&self) -> i32 {
        match self {
            Dimension::Overworld => 63,
            Dimension::Nether => 32,
            Dimension::End => 0,
        }
    }
}

#[derive(Clone)]
pub struct TerrainGenerator {
    pub dimension: Dimension,
    pub seed: u32,
    height_noise: Fbm<SuperSimplex>,
    detail_noise: Fbm<SuperSimplex>,
    cave_noise: Fbm<SuperSimplex>,
    biome_noise: SuperSimplex,
    ore_noise: Fbm<Perlin>,
}

impl TerrainGenerator {
    pub fn new(dimension: Dimension, seed: u32) -> Self {
        let mut height_noise = Fbm::<SuperSimplex>::new(seed);
        height_noise.octaves = 6;
        height_noise.frequency = 0.005;
        height_noise.lacunarity = 2.0;
        height_noise.persistence = 0.5;

        let mut detail_noise = Fbm::<SuperSimplex>::new(seed.wrapping_add(1));
        detail_noise.octaves = 4;
        detail_noise.frequency = 0.02;
        detail_noise.lacunarity = 2.0;
        detail_noise.persistence = 0.5;

        let mut cave_noise = Fbm::<SuperSimplex>::new(seed.wrapping_add(2));
        cave_noise.octaves = 3;
        cave_noise.frequency = 0.03;
        cave_noise.lacunarity = 2.0;
        cave_noise.persistence = 0.5;

        let biome_noise = SuperSimplex::new(seed.wrapping_add(3));

        let mut ore_noise = Fbm::<Perlin>::new(seed.wrapping_add(4));
        ore_noise.octaves = 2;
        ore_noise.frequency = 0.1;
        ore_noise.lacunarity = 2.0;
        ore_noise.persistence = 0.5;

        Self {
            dimension,
            seed,
            height_noise,
            detail_noise,
            cave_noise,
            biome_noise,
            ore_noise,
        }
    }

    pub fn generate_chunk(&self, chunk_x: i32, chunk_z: i32) -> UnloadedChunk {
        match self.dimension {
            Dimension::Overworld => self.generate_overworld_chunk(chunk_x, chunk_z),
            Dimension::Nether => self.generate_nether_chunk(chunk_x, chunk_z),
            Dimension::End => self.generate_end_chunk(chunk_x, chunk_z),
        }
    }

    fn generate_overworld_chunk(&self, chunk_x: i32, chunk_z: i32) -> UnloadedChunk {
        let mut chunk = UnloadedChunk::with_height(384);

        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        for local_x in 0..16 {
            for local_z in 0..16 {
                let world_x = base_x + local_x as i32;
                let world_z = base_z + local_z as i32;

                let height = self.overworld_height(world_x, world_z);
                let biome = self.overworld_biome(world_x, world_z);

                for world_y in -64..=height {
                    let chunk_y = (world_y + 64) as u32;
                    let block = if world_y == height {
                        match biome {
                            OverworldBiome::Desert => BlockState::SAND,
                            OverworldBiome::Beach => BlockState::SAND,
                            OverworldBiome::Ocean => BlockState::SAND,
                            _ => BlockState::GRASS_BLOCK,
                        }
                    } else if world_y >= height - 3 {
                        match biome {
                            OverworldBiome::Desert => BlockState::SAND,
                            OverworldBiome::Beach => BlockState::SAND,
                            _ => BlockState::DIRT,
                        }
                    } else if world_y >= -64 && world_y < -60 {
                        BlockState::BEDROCK
                    } else {
                        let cave = self.cave_noise.get([
                            world_x as f64 * 0.05,
                            world_y as f64 * 0.05,
                            world_z as f64 * 0.05,
                        ]);
                        if cave > 0.5 && world_y > -60 && world_y < height - 5 {
                            BlockState::AIR
                        } else {
                            let ore = self.ore_noise.get([
                                world_x as f64 * 0.1,
                                world_y as f64 * 0.1,
                                world_z as f64 * 0.1,
                            ]);
                            if world_y < 16 && ore > 1.2 {
                                BlockState::DIAMOND_ORE
                            } else if world_y < 32 && ore > 1.0 {
                                BlockState::IRON_ORE
                            } else if world_y < 48 && ore > 0.8 {
                                BlockState::COAL_ORE
                            } else {
                                BlockState::STONE
                            }
                        }
                    };

                    chunk.set_block(local_x as u32, chunk_y, local_z as u32, block);
                }

                let sea = self.dimension.sea_level();
                if height < sea {
                    for world_y in (height + 1)..=sea {
                        let chunk_y = (world_y + 64) as u32;
                        chunk.set_block(local_x as u32, chunk_y, local_z as u32, BlockState::WATER);
                    }
                }

                if height >= sea {
                    self.place_overworld_decoration(
                        &mut chunk,
                        local_x,
                        height,
                        local_z,
                        biome,
                    );
                }
            }
        }

        chunk
    }

    fn overworld_height(&self, x: i32, z: i32) -> i32 {
        let base = self.height_noise.get([x as f64, z as f64]) * 40.0;
        let detail = self.detail_noise.get([x as f64, z as f64]) * 8.0;
        let biome_val = self.biome_noise.get([x as f64 * 0.002, z as f64 * 0.002]);

        let height = 64.0 + base + detail;

        if biome_val > 0.3 {
            (height + biome_val * 20.0) as i32
        } else if biome_val < -0.3 {
            (height - 5.0) as i32
        } else {
            height as i32
        }
    }

    fn overworld_biome(&self, x: i32, z: i32) -> OverworldBiome {
        let temp = self.biome_noise.get([x as f64 * 0.003, z as f64 * 0.003]);
        let height = self.overworld_height(x, z);
        let sea = self.dimension.sea_level();

        if height < sea - 3 {
            OverworldBiome::Ocean
        } else if height < sea + 2 {
            OverworldBiome::Beach
        } else if temp > 0.5 {
            OverworldBiome::Desert
        } else if temp > 0.2 {
            OverworldBiome::Forest
        } else if temp < -0.3 {
            OverworldBiome::Taiga
        } else {
            OverworldBiome::Plains
        }
    }

    fn place_overworld_decoration(
        &self,
        chunk: &mut UnloadedChunk,
        local_x: usize,
        height: i32,
        local_z: usize,
        biome: OverworldBiome,
    ) {
        let mut rng = rand::thread_rng();
        let grass_chance = match biome {
            OverworldBiome::Plains => 0.3,
            OverworldBiome::Forest => 0.2,
            OverworldBiome::Taiga => 0.1,
            _ => 0.0,
        };

        if rng.gen::<f32>() < grass_chance {
            let tall_grass_height = if rng.gen::<f32>() < 0.1 { 2 } else { 1 };
            for dy in 0..tall_grass_height {
                let chunk_y = (height + 1 + dy + 64) as u32;
                chunk.set_block(local_x as u32, chunk_y, local_z as u32, BlockState::GRASS);
            }
        }

        let tree_chance = match biome {
            OverworldBiome::Forest => 0.02,
            OverworldBiome::Taiga => 0.015,
            OverworldBiome::Plains => 0.002,
            _ => 0.0,
        };

        if rng.gen::<f32>() < tree_chance
            && local_x >= 2
            && local_x <= 13
            && local_z >= 2
            && local_z <= 13
        {
            let tree_height = match biome {
                OverworldBiome::Taiga => 7 + rng.gen_range(0..4),
                _ => 4 + rng.gen_range(0..3),
            };

            let trunk = match biome {
                OverworldBiome::Taiga => BlockState::SPRUCE_LOG,
                _ => BlockState::OAK_LOG,
            };
            let leaves = match biome {
                OverworldBiome::Taiga => BlockState::SPRUCE_LEAVES,
                _ => BlockState::OAK_LEAVES,
            };

            for dy in 1..=tree_height {
                let chunk_y = (height + dy + 64) as u32;
                chunk.set_block(local_x as u32, chunk_y, local_z as u32, trunk);
            }

            let leaf_start = tree_height - 2;
            for dy in leaf_start..=tree_height + 1 {
                let radius = if dy > tree_height { 1 } else { 2 };
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        if dx == 0 && dz == 0 && dy <= tree_height {
                            continue;
                        }
                        let lx = local_x as i32 + dx;
                        let lz = local_z as i32 + dz;
                        if lx >= 0 && lx < 16 && lz >= 0 && lz < 16 {
                            let chunk_y = (height + dy + 64) as u32;
                            chunk.set_block(lx as u32, chunk_y, lz as u32, leaves);
                        }
                    }
                }
            }
        }

        if biome == OverworldBiome::Desert && rng.gen::<f32>() < 0.005 {
            let cactus_height = 1 + rng.gen_range(0..3);
            for dy in 0..cactus_height {
                let chunk_y = (height + 1 + dy + 64) as u32;
                chunk.set_block(local_x as u32, chunk_y, local_z as u32, BlockState::CACTUS);
            }
        }

        if biome == OverworldBiome::Plains && rng.gen::<f32>() < 0.008 {
            let flower = match rng.gen_range(0..4) {
                0 => BlockState::POPPY,
                1 => BlockState::DANDELION,
                2 => BlockState::CORNFLOWER,
                _ => BlockState::AZURE_BLUET,
            };
            let chunk_y = (height + 1 + 64) as u32;
            chunk.set_block(local_x as u32, chunk_y, local_z as u32, flower);
        }
    }

    fn generate_nether_chunk(&self, chunk_x: i32, chunk_z: i32) -> UnloadedChunk {
        let mut chunk = UnloadedChunk::with_height(128);
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        for local_x in 0..16 {
            for local_z in 0..16 {
                let world_x = base_x + local_x as i32;
                let world_z = base_z + local_z as i32;

                for y in 0..128 {
                    let density = self.nether_density(world_x, y, world_z);
                    let block = if y < 5 || y > 122 {
                        BlockState::BEDROCK
                    } else if density > 0.0 {
                        let ore = self.ore_noise.get([
                            world_x as f64 * 0.1,
                            y as f64 * 0.1,
                            world_z as f64 * 0.1,
                        ]);
                        if ore > 1.3 && y < 30 {
                            BlockState::ANCIENT_DEBRIS
                        } else if ore > 1.1 && y < 80 {
                            BlockState::NETHER_QUARTZ_ORE
                        } else if ore > 0.9 && y < 40 {
                            BlockState::NETHER_GOLD_ORE
                        } else {
                            BlockState::NETHERRACK
                        }
                    } else if y < 32 {
                        BlockState::LAVA
                    } else {
                        BlockState::AIR
                    };

                    chunk.set_block(local_x as u32, y as u32, local_z as u32, block);
                }
            }
        }

        chunk
    }

    fn nether_density(&self, x: i32, y: i32, z: i32) -> f64 {
        let n1 = self
            .height_noise
            .get([x as f64 * 0.02, y as f64 * 0.02, z as f64 * 0.02]);
        let n2 = self
            .detail_noise
            .get([x as f64 * 0.05, y as f64 * 0.05, z as f64 * 0.05]);

        let center_dist = ((y as f64 - 64.0) / 64.0).min(1.0).max(-1.0);
        let edge_factor = 1.0 - center_dist * center_dist;

        (n1 + n2 * 0.5) * edge_factor
    }

    fn generate_end_chunk(&self, chunk_x: i32, chunk_z: i32) -> UnloadedChunk {
        let mut chunk = UnloadedChunk::with_height(128);
        let base_x = chunk_x * 16;
        let base_z = chunk_z * 16;

        let dist_from_origin = ((chunk_x as f64).hypot(chunk_z as f64) * 16.0) as f64;

        for local_x in 0..16 {
            for local_z in 0..16 {
                let world_x = base_x + local_x as i32;
                let world_z = base_z + local_z as i32;

                let end_noise = self
                    .height_noise
                    .get([world_x as f64 * 0.01, world_z as f64 * 0.01]);
                let island_density = self
                    .detail_noise
                    .get([world_x as f64 * 0.005, world_z as f64 * 0.005]);

                let is_main_island = dist_from_origin < 500.0;
                let is_outer_island = island_density > 0.2 && dist_from_origin > 800.0;

                if is_main_island || is_outer_island {
                    let height = if is_main_island {
                        let base = 64.0 + end_noise * 10.0;
                        if dist_from_origin < 100.0 {
                            base
                        } else {
                            let fade = 1.0 - (dist_from_origin - 100.0) / 400.0;
                            base * fade.max(0.0)
                        }
                    } else {
                        64.0 + end_noise * 15.0
                    };

                    if height > 60.0 {
                        let h = height as i32;
                        for y in 60..=h {
                            let block = if y == h {
                                BlockState::END_STONE
                            } else {
                                BlockState::END_STONE
                            };
                            chunk.set_block(local_x as u32, y as u32, local_z as u32, block);
                        }
                    }
                }
            }
        }

        chunk
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OverworldBiome {
    Plains,
    Forest,
    Desert,
    Taiga,
    Beach,
    Ocean,
}

pub fn generate_overworld_terrain(layer: &mut ChunkLayer, chunk_x: i32, chunk_z: i32, seed: u32) {
    let gen = TerrainGenerator::new(Dimension::Overworld, seed);
    let chunk_data = gen.generate_chunk(chunk_x, chunk_z);

    for local_x in 0..16u32 {
        for local_z in 0..16u32 {
            for y in 0..chunk_data.height() {
                let state = chunk_data.block_state(local_x, y, local_z);
                if state != BlockState::AIR {
                    let pos = BlockPos::new(
                        chunk_x * 16 + local_x as i32,
                        y as i32 - 64,
                        chunk_z * 16 + local_z as i32,
                    );
                    layer.set_block(pos, state);
                }
            }
        }
    }
}

pub fn generate_nether_terrain(layer: &mut ChunkLayer, chunk_x: i32, chunk_z: i32, seed: u32) {
    let gen = TerrainGenerator::new(Dimension::Nether, seed);
    let chunk_data = gen.generate_chunk(chunk_x, chunk_z);

    for local_x in 0..16u32 {
        for local_z in 0..16u32 {
            for y in 0..chunk_data.height() {
                let state = chunk_data.block_state(local_x, y, local_z);
                if state != BlockState::AIR {
                    let pos = BlockPos::new(
                        chunk_x * 16 + local_x as i32,
                        y as i32,
                        chunk_z * 16 + local_z as i32,
                    );
                    layer.set_block(pos, state);
                }
            }
        }
    }
}

pub fn generate_end_terrain(layer: &mut ChunkLayer, chunk_x: i32, chunk_z: i32, seed: u32) {
    let gen = TerrainGenerator::new(Dimension::End, seed);
    let chunk_data = gen.generate_chunk(chunk_x, chunk_z);

    for local_x in 0..16u32 {
        for local_z in 0..16u32 {
            for y in 0..chunk_data.height() {
                let state = chunk_data.block_state(local_x, y, local_z);
                if state != BlockState::AIR {
                    let pos = BlockPos::new(
                        chunk_x * 16 + local_x as i32,
                        y as i32,
                        chunk_z * 16 + local_z as i32,
                    );
                    layer.set_block(pos, state);
                }
            }
        }
    }
}