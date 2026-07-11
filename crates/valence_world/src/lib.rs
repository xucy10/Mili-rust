#![allow(
    clippy::doc_markdown,
    clippy::impl_trait_in_params,
    clippy::cast_lossless,
    clippy::uninlined_format_args,
    clippy::map_unwrap_or,
    clippy::field_reassign_with_default,
    clippy::ref_patterns
)]

pub mod level_dat;
pub mod save_system;

pub use level_dat::{LevelDat, LevelData};
pub use save_system::{WorldSaveManager, WorldSavePlugin};
