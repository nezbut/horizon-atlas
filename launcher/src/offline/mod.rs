//! Atlas's own accounts and offline play. The official launcher shares the data folder and its
//! current code rewrites `accounts.json` and `skins/` there, so everything Atlas keeps lives in
//! the `atlas/` folder inside it: its accounts and their secrets, the offline names with their
//! skins and capes, and who plays.
pub mod cli;
pub mod migrate;
pub mod profiles;
pub mod session;
pub mod skins;

pub use session::session;

use std::path::{Path, PathBuf};

/// Atlas's folder inside the data folder `data`.
pub fn atlas_in(data: &Path) -> PathBuf {
    data.join("atlas")
}

/// Atlas's folder in the launcher's data folder.
pub fn atlas_dir() -> PathBuf {
    atlas_in(&crate::backend::data_dir())
}

#[cfg(test)]
mod tests;
