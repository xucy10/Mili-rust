#![allow(clippy::absurd_extreme_comparisons)]

pub mod level_dat;
pub mod save_system;

pub use level_dat::{LevelDat, LevelData};
pub use save_system::{WorldSaveManager, WorldSavePlugin};
