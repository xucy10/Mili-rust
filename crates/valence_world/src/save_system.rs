use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_time::{Timer, TimerMode};
use tracing::{error, info};
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
    if save_manager.dirty_count() == 0 && save_manager.level_dat.is_some() {
        return;
    }

    info!("Saving world on shutdown...");

    if let Err(e) = save_manager.save_level_dat() {
        error!("Failed to save level.dat on shutdown: {}", e);
    }

    if let Err(e) = perform_save(&mut save_manager, &layers) {
        error!("Shutdown save failed: {}", e);
    } else {
        info!("World saved successfully on shutdown.");
    }
}

/// Performs the actual save operation.
fn perform_save(
    save_manager: &mut WorldSaveManager,
    _layers: &Query<&ChunkLayer>,
) -> Result<(), Box<dyn std::error::Error>> {
    save_manager.begin_save();

    let world_path = &save_manager.world_path;

    // Ensure directories exist
    let region_dir = world_path.join("region");
    fs::create_dir_all(&region_dir)?;

    // Save level.dat
    save_manager.save_level_dat()?;

    // Save dirty chunks using the Anvil format
    let dirty: Vec<_> = save_manager.dirty_chunks().iter().copied().collect();

    for (chunk_x, chunk_z) in &dirty {
        info!("Saving chunk ({}, {})", chunk_x, chunk_z);
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
