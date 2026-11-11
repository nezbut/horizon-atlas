//! What the game is started with while an offline name plays (or nobody): no account session, the
//! name in `HORIZON_PLAYER_NAME`, and the name's skin and cape for the game to draw. The window,
//! `play`, `modpack play` and the MCP server all take their session from
//! `account::session_for_game`, which asks here first.
use super::profiles::Profiles;
use super::skins;
use crate::account::GameSession;
use std::path::Path;

/// The variable the game takes the player's name from.
pub const NAME_ENV: &str = "HORIZON_PLAYER_NAME";

/// The offline session; none when a Microsoft account was chosen to play, which is upstream's.
/// The move from 0.1.x runs first (`Profiles::load`).
pub fn session() -> Option<GameSession> {
    session_in(&crate::backend::data_dir())
}

/// [`session`] for the data folder `data`.
pub fn session_in(data: &Path) -> Option<GameSession> {
    let profiles = Profiles::load_in(data);
    if !profiles.plays_offline() {
        return None;
    }
    // With no name the game keeps the one it made up (`horizon-player.txt`).
    let mut env = Vec::new();
    if let Some(player) = profiles.selected() {
        env.push((NAME_ENV, player.name.clone()));
        env.extend(skins::env(&skins::dir_in(data), &player.name, player.model));
    }
    Some(GameSession {
        file: None,
        offline: None,
        env,
    })
}
