use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_time::{Time, Timer, TimerMode};
use tracing::{error, info};
use valence_anvil::{RegionFolder, WriteOptions};
use valence_nbt::{Compound, List, Value};
use valence_protocol::BlockState;
use valence_registry::biome::BiomeId;
use valence_server::layer::chunk::Chunk;
use valence_server::ChunkLayer;

use crate::level_dat::{LevelDat, LevelDatError};

/// Resource for managing world save operations.
#[derive(Resource)]
pub struct WorldSaveManager {
    /// Root path of the world directory.
    pub world_path: PathBuf,
    /// Level.dat data.
    pub level_dat: Option<LevelDat>,
    /// Dirty chunks that need to be saved.
    dirty_chunks: HashSet<(i32, i32)>,
    /// Timer for auto-save interval.
    auto_save_timer: Timer,
    /// Whether auto-save is enabled.
    pub auto_save_enabled: bool,
    /// Auto-save interval in seconds.
    pub auto_save_interval_secs: u64,
    /// Whether a save is currently in progress.
    saving_in_progress: bool,
    /// Whether shutdown save has been performed.
    shutdown_saved: bool,
}

impl WorldSaveManager {
    /// Creates a new WorldSaveManager for the given world path.
    pub fn new(world_path: impl Into<PathBuf>) -> Self {
        Self {
            world_path: world_path.into(),
            level_dat: None,
            dirty_chunks: HashSet::new(),
            auto_save_timer: Timer::from_seconds(300.0, TimerMode::Repeating),
            auto_save_enabled: true,
            auto_save_interval_secs: 300,
            saving_in_progress: false,
            shutdown_saved: false,
        }
    }

    /// Creates a new WorldSaveManager with custom auto-save interval.
    pub fn with_auto_save_interval(world_path: impl Into<PathBuf>, interval_secs: u64) -> Self {
        Self {
            auto_save_timer: Timer::from_seconds(interval_secs as f32, TimerMode::Repeating),
            auto_save_interval_secs: interval_secs,
            ..Self::new(world_path)
        }
    }

    /// Loads the level.dat file if it exists.
    pub fn load_level_dat(&mut self) -> Result<(), LevelDatError> {
        let path = self.world_path.join("level.dat");
        if path.exists() {
            self.level_dat = Some(LevelDat::read(&path)?);
            info!("Loaded level.dat from {}", path.display());
        } else {
            self.level_dat = Some(LevelDat::new());
            info!("Created new level.dat at {}", path.display());
        }
        Ok(())
    }

    /// Saves the level.dat file.
    pub fn save_level_dat(&self) -> Result<(), LevelDatError> {
        if let Some(level_dat) = &self.level_dat {
            let path = self.world_path.join("level.dat");
            level_dat.write(&path)?;
            info!("Saved level.dat to {}", path.display());
        }
        Ok(())
    }

    /// Marks a chunk as dirty (needs saving).
    pub fn mark_dirty(&mut self, x: i32, z: i32) {
        self.dirty_chunks.insert((x, z));
    }

    /// Removes a chunk from the dirty set.
    pub fn mark_clean(&mut self, x: i32, z: i32) {
        self.dirty_chunks.remove(&(x, z));
    }

    /// Returns the number of dirty chunks.
    pub fn dirty_count(&self) -> usize {
        self.dirty_chunks.len()
    }

    /// Returns a reference to the dirty chunk positions.
    pub fn dirty_chunks(&self) -> &HashSet<(i32, i32)> {
        &self.dirty_chunks
    }

    /// Clears all dirty chunks (after saving).
    pub fn clear_dirty(&mut self) {
        self.dirty_chunks.clear();
    }

    /// Sets the auto-save interval in seconds.
    pub fn set_auto_save_interval(&mut self, interval_secs: u64) {
        self.auto_save_interval_secs = interval_secs;
        self.auto_save_timer = Timer::from_seconds(interval_secs as f32, TimerMode::Repeating);
    }

    /// Enables or disables auto-save.
    pub fn set_auto_save_enabled(&mut self, enabled: bool) {
        self.auto_save_enabled = enabled;
    }

    /// Returns true if auto-save is enabled and the timer has elapsed.
    pub fn should_auto_save(&self) -> bool {
        self.auto_save_enabled && self.auto_save_timer.finished()
    }

    /// Resets the auto-save timer.
    pub fn reset_auto_save_timer(&mut self) {
        self.auto_save_timer.reset();
    }

    /// Checks if a save operation is in progress.
    pub fn is_saving(&self) -> bool {
        self.saving_in_progress
    }

    /// Marks a save as in progress.
    pub fn begin_save(&mut self) {
        self.saving_in_progress = true;
    }

    /// Marks a save as completed.
    pub fn end_save(&mut self) {
        self.saving_in_progress = false;
    }
}

impl Default for WorldSaveManager {
    fn default() -> Self {
        Self::new("world")
    }
}

/// System that updates the auto-save timer.
pub fn update_save_timer(mut save_manager: ResMut<WorldSaveManager>, time: Res<Time>) {
    if save_manager.auto_save_enabled {
        save_manager.auto_save_timer.tick(time.delta());
    }
}

/// System that performs auto-save when the timer elapses.
pub fn auto_save_system(mut save_manager: ResMut<WorldSaveManager>, layers: Query<&ChunkLayer>) {
    if !save_manager.should_auto_save() {
        return;
    }

    save_manager.reset_auto_save_timer();

    if save_manager.dirty_count() == 0 {
        return;
    }

    info!(
        "Auto-saving world ({} dirty chunks)...",
        save_manager.dirty_count()
    );

    if let Err(e) = perform_save(&mut save_manager, &layers) {
        error!("Auto-save failed: {}", e);
    }
}

/// System that saves the world on shutdown.
pub fn shutdown_save_system(
    mut save_manager: ResMut<WorldSaveManager>,
    layers: Query<&ChunkLayer>,
) {
    if save_manager.shutdown_saved {
        return;
    }

    info!("Saving world on shutdown...");

    let _ = save_manager.save_level_dat();

    if let Err(e) = perform_save(&mut save_manager, &layers) {
        error!("Shutdown save failed: {}", e);
    } else {
        info!("World saved successfully on shutdown.");
    }

    save_manager.shutdown_saved = true;
}

/// Performs the actual save operation.
fn perform_save(
    save_manager: &mut WorldSaveManager,
    layers: &Query<&ChunkLayer>,
) -> Result<(), Box<dyn std::error::Error>> {
    save_manager.begin_save();

    let world_path = &save_manager.world_path;

    // Ensure directories exist
    let region_dir = world_path.join("region");
    fs::create_dir_all(&region_dir)?;

    // Save level.dat
    save_manager.save_level_dat()?;

    // Save chunks using the Anvil format
    for layer in layers {
        let mut region_folder = RegionFolder::new(&region_dir);
        region_folder.write_options = WriteOptions::default();
                region_folder.write_options.compression = valence_anvil::Compression::Zlib;

        for (chunk_pos, chunk) in layer.chunks() {
            let chunk_x = chunk_pos.x;
            let chunk_z = chunk_pos.z;

            let nbt = chunk_to_anvil_compound(chunk, chunk_x, chunk_z, layer.min_y(), layer.height());
            match nbt {
                Ok(compound) => {
                    region_folder.set_chunk(chunk_x, chunk_z, &compound)?;
                    info!("Saved chunk ({}, {})", chunk_x, chunk_z);
                }
                Err(e) => {
                    error!("Failed to serialize chunk ({}, {}): {}", chunk_x, chunk_z, e);
                }
            }
        }
    }

    save_manager.clear_dirty();
    save_manager.end_save();

    Ok(())
}

/// Triggers a manual world save. Returns Ok(()) on success.
pub fn trigger_save(
    save_manager: &mut WorldSaveManager,
    layers: &Query<&ChunkLayer>,
) -> Result<(), String> {
    if let Err(e) = save_manager.save_level_dat() {
        return Err(format!("Failed to save level.dat: {}", e));
    }

    if let Err(e) = perform_save(save_manager, layers) {
        return Err(format!("Save failed: {}", e));
    }

    Ok(())
}

/// Plugin that manages world saving.
pub struct WorldSavePlugin;

impl Plugin for WorldSavePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (update_save_timer, auto_save_system).chain())
            .add_systems(Last, shutdown_save_system);
    }
}

/// Helper function to create a WorldSaveManager from a path.
pub fn create_save_manager(world_path: impl Into<PathBuf>) -> WorldSaveManager {
    WorldSaveManager::new(world_path)
}

/// Helper function to create a WorldSaveManager with default settings.
pub fn default_save_manager() -> WorldSaveManager {
    WorldSaveManager::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_tracking() {
        let mut manager = WorldSaveManager::new("test_world");
        assert_eq!(manager.dirty_count(), 0);

        manager.mark_dirty(0, 0);
        assert_eq!(manager.dirty_count(), 1);

        manager.mark_dirty(1, 1);
        assert_eq!(manager.dirty_count(), 2);

        manager.mark_clean(0, 0);
        assert_eq!(manager.dirty_count(), 1);

        manager.clear_dirty();
        assert_eq!(manager.dirty_count(), 0);
    }

    #[test]
    fn test_auto_save_config() {
        let mut manager = WorldSaveManager::new("test_world");
        assert!(manager.auto_save_enabled);
        assert_eq!(manager.auto_save_interval_secs, 300);

        manager.set_auto_save_enabled(false);
        assert!(!manager.auto_save_enabled);

        manager.set_auto_save_interval(60);
        assert_eq!(manager.auto_save_interval_secs, 60);
    }
}

/// Converts a chunk to the Anvil NBT compound format.
fn chunk_to_anvil_compound(
    chunk: &valence_server::layer::chunk::LoadedChunk,
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: u32,
) -> Result<Compound, String> {
    let mut root = Compound::new();

    root.insert("DataVersion".to_owned(), Value::Int(3715));
    root.insert("xPos".to_owned(), Value::Int(chunk_x));
    root.insert("zPos".to_owned(), Value::Int(chunk_z));
    root.insert("yPos".to_owned(), Value::Int(min_y / 16));
    root.insert("Status".to_owned(), Value::String("full".to_owned()));

    let section_count = height / 16;
    let mut sections: Vec<Compound> = Vec::new();

    for sect_y in 0..section_count {
        let mut section_compound = Compound::new();
        let byte_y = (sect_y as i32 + (min_y / 16)) as i8;
        section_compound.insert("Y".to_owned(), Value::Byte(byte_y));

        // Block states
        let mut block_palette: Vec<BlockState> = Vec::new();
        let mut block_indices = Vec::with_capacity(4096);

        for y in 0..16u32 {
            for z in 0..16u32 {
                for x in 0..16u32 {
                    let state = chunk.block_state(x, sect_y * 16 + y, z);
                    let idx = match block_palette.iter().position(|s| *s == state) {
                        Some(i) => i,
                        None => {
                            let i = block_palette.len();
                            block_palette.push(state);
                            i
                        }
                    };
                    block_indices.push(idx as u64);
                }
            }
        }

        let block_palette_nbt: Vec<Compound> = block_palette
            .iter()
            .map(|state| {
                let mut block = Compound::new();
                let name = state.to_string();
                block.insert("Name".to_owned(), Value::String(name));
                block
            })
            .collect();

        let mut block_states = Compound::new();
        block_states.insert(
            "palette".to_owned(),
            Value::List(List::Compound(block_palette_nbt)),
        );

        if block_palette.len() > 1 {
            let bit_width = std::cmp::max(1, ceil_log2(block_palette.len() as u64));
            let packed = pack_indices(&block_indices, bit_width);
            block_states.insert("data".to_owned(), Value::LongArray(packed));
        }

        section_compound.insert(
            "block_states".to_owned(),
            Value::Compound(block_states),
        );

        // Biomes
        let mut biome_palette: Vec<BiomeId> = Vec::new();
        let mut biome_indices = Vec::with_capacity(64);

        for y in 0..4u32 {
            for z in 0..4u32 {
                for x in 0..4u32 {
                    let biome = chunk.biome(x, sect_y * 4 + y, z);
                    let idx = match biome_palette.iter().position(|b| *b == biome) {
                        Some(i) => i,
                        None => {
                            let i = biome_palette.len();
                            biome_palette.push(biome);
                            i
                        }
                    };
                    biome_indices.push(idx as u64);
                }
            }
        }

        let biome_palette_nbt: Vec<String> = biome_palette
            .iter()
            .map(|_b| "minecraft:plains".to_owned())
            .collect();

        let mut biomes = Compound::new();
        biomes.insert(
            "palette".to_owned(),
            Value::List(List::String(biome_palette_nbt)),
        );

        if biome_palette.len() > 1 {
            let bit_width = std::cmp::max(1, ceil_log2(biome_palette.len() as u64));
            let packed = pack_indices(&biome_indices, bit_width);
            biomes.insert("data".to_owned(), Value::LongArray(packed));
        }

        section_compound.insert("biomes".to_owned(), Value::Compound(biomes));

        sections.push(section_compound);
    }

    root.insert(
        "sections".to_owned(),
        Value::List(List::Compound(sections)),
    );

    // Heightmaps (empty for now)
    let heightmaps = Compound::new();
    root.insert("Heightmaps".to_owned(), Value::Compound(heightmaps));

    // Block entities (empty list)
    let block_entities_list: Vec<Compound> = Vec::new();
    root.insert(
        "block_entities".to_owned(),
        Value::List(List::Compound(block_entities_list)),
    );

    Ok(root)
}

/// Packs indices into a long array using the given bit width.
fn pack_indices(indices: &[u64], bit_width: u32) -> Vec<i64> {
    let values_per_long = 64 / bit_width;
    let mask = (1u64 << bit_width) - 1;
    let num_longs = indices.len().div_ceil(values_per_long as usize);

    let mut packed = vec![0i64; num_longs];

    for (i, &idx) in indices.iter().enumerate() {
        let long_idx = i / values_per_long as usize;
        let bit_offset = (i % values_per_long as usize) as u32 * bit_width;
        packed[long_idx] |= ((idx & mask) as i64) << bit_offset;
    }

    packed
}

/// Computes ceil(log2(n)).
fn ceil_log2(n: u64) -> u32 {
    if n <= 1 {
        return 1;
    }
    64 - (n - 1).leading_zeros()
}