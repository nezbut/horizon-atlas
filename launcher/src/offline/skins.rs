//! Offline skins and capes: a PNG the player picks for an offline name, kept in Atlas's `skins/`
//! folder, with the model (classic arms or slim) that the name's profile records. The game gets
//! them in `HORIZON_OFFLINE_SKIN`, `HORIZON_OFFLINE_SKIN_MODEL` and `HORIZON_OFFLINE_CAPE` when
//! that name plays, and draws them on the local player (`skins::offline_skin`, `offline_cape`).
//!
//! Skins are the layout every Java Edition skin uses: 64×64, or the older 64×32 (whose left arm
//! and leg are the right ones mirrored). Capes are 64×32 (or the older 22×17).
use crate::i18n::{tr, trf};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SKIN_ENV: &str = "HORIZON_OFFLINE_SKIN";
pub const MODEL_ENV: &str = "HORIZON_OFFLINE_SKIN_MODEL";
pub const CAPE_ENV: &str = "HORIZON_OFFLINE_CAPE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    /// Four-pixel arms ("Steve").
    #[default]
    Classic,
    /// Three-pixel arms ("Alex").
    Slim,
}

impl Model {
    pub fn name(self) -> &'static str {
        match self {
            Model::Classic => "classic",
            Model::Slim => "slim",
        }
    }
}

/// Where an offline name's files are: `<name>.png` and `<name>.cape.png` in `dir`, the name in
/// lowercase (offline names are kept once in any case). A name that was never valid (a hand-edited
/// file) can't point outside `dir`: anything but letters, digits and `_` becomes `_`.
pub fn files(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let key: String = name
        .chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' => c.to_ascii_lowercase(),
            _ => '_',
        })
        .collect();
    (
        dir.join(format!("{key}.png")),
        dir.join(format!("{key}.cape.png")),
    )
}

/// Atlas's skins folder in the data folder `data`.
pub fn dir_in(data: &Path) -> PathBuf {
    super::atlas_in(data).join("skins")
}

pub fn dir() -> PathBuf {
    dir_in(&crate::backend::data_dir())
}

fn decode(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .map_err(|_| tr("That isn't a PNG picture.").to_string())
}

/// A skin's picture, if it is one: 64×64 or 64×32.
pub fn read_skin(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let png = decode(bytes)?;
    match png.dimensions() {
        (64, 64) | (64, 32) => Ok(png),
        (w, h) => Err(trf(
            "A skin is 64×64 (or the older 64×32) pixels; this one is {w}×{h}.",
            &[("w", &w.to_string()), ("h", &h.to_string())],
        )),
    }
}

/// A cape's picture, if it is one: 64×32 or 22×17.
pub fn read_cape(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let png = decode(bytes)?;
    match png.dimensions() {
        (64, 32) | (22, 17) => Ok(png),
        (w, h) => Err(trf(
            "A cape is 64×32 (or the older 22×17) pixels; this one is {w}×{h}.",
            &[("w", &w.to_string()), ("h", &h.to_string())],
        )),
    }
}

/// A part of the skin copied to the front view: from (x, y), its size, to (x, y), mirrored.
type Part = (u32, u32, u32, u32, u32, u32, bool);

/// The front of the player, 16×32 texels: head, body, arms and legs, each with its outer layer
/// over it (64×64 skins have one for every part; 64×32 only for the head).
pub fn front(skin: &image::RgbaImage, model: Model) -> Vec<[u8; 4]> {
    let w = 16usize;
    let mut out = vec![[0u8; 4]; w * 32];
    let modern = skin.height() == 64;
    let arm = if model == Model::Slim { 3 } else { 4 };
    let mut base: Vec<Part> = vec![
        (8, 8, 8, 8, 4, 0, false),            // head
        (20, 20, 8, 12, 4, 8, false),         // body
        (44, 20, arm, 12, 4 - arm, 8, false), // right arm, on the viewer's left
        (4, 20, 4, 12, 4, 20, false),         // right leg
    ];
    let mut outer: Vec<Part> = vec![(40, 8, 8, 8, 4, 0, false)];
    if modern {
        base.extend([
            (36, 52, arm, 12, 12, 8, false),
            (20, 52, 4, 12, 8, 20, false),
        ]);
        outer.extend([
            (20, 36, 8, 12, 4, 8, false),
            (44, 36, arm, 12, 4 - arm, 8, false),
            (52, 52, arm, 12, 12, 8, false),
            (4, 36, 4, 12, 4, 20, false),
            (4, 52, 4, 12, 8, 20, false),
        ]);
    } else {
        base.extend([(44, 20, arm, 12, 12, 8, true), (4, 20, 4, 12, 8, 20, true)]);
    }
    for (layer, parts) in [base, outer].into_iter().enumerate() {
        for (fx, fy, pw, ph, tx, ty, mirrored) in parts {
            for dy in 0..ph {
                for dx in 0..pw {
                    let sx = if mirrored { fx + pw - 1 - dx } else { fx + dx };
                    let c = skin.get_pixel(sx, fy + dy).0;
                    // The base layer is opaque in game; the outer one shows where it is drawn.
                    if layer == 0 || c[3] > 0 {
                        out[(ty + dy) as usize * w + (tx + dx) as usize] = [c[0], c[1], c[2], 255];
                    }
                }
            }
        }
    }
    out
}

/// The cape's outside, 10×16 texels.
pub fn cape_front(cape: &image::RgbaImage) -> Vec<[u8; 4]> {
    let mut out = Vec::with_capacity(160);
    for y in 1..17 {
        for x in 1..11 {
            out.push(cape.get_pixel(x, y).0);
        }
    }
    out
}

/// Keeps `file` as `name`'s skin (or cape), checked first.
pub fn keep(dir: &Path, name: &str, file: &Path, cape: bool) -> Result<(), String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    match cape {
        false => read_skin(&bytes).map(|_| ())?,
        true => read_cape(&bytes).map(|_| ())?,
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let (skin, cape_file) = files(dir, name);
    let to = if cape { cape_file } else { skin };
    std::fs::write(&to, bytes).map_err(|e| format!("{}: {e}", to.display()))
}

/// Drops `name`'s skin (or cape). A file that isn't there is already gone; any other failure is
/// an error naming the file, since a file left behind is still sent to the game.
pub fn clear(dir: &Path, name: &str, cape: bool) -> Result<(), String> {
    let (skin, cape_file) = files(dir, name);
    let file = if cape { cape_file } else { skin };
    match std::fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(trf(
            "Could not remove {file}: {error}",
            &[
                ("file", &file.display().to_string()),
                ("error", &e.to_string()),
            ],
        )),
        _ => Ok(()),
    }
}

/// Drops a forgotten name's skin and cape. Both are tried; the first failure is the answer.
pub fn forget(dir: &Path, name: &str) -> Result<(), String> {
    let skin = clear(dir, name, false);
    let cape = clear(dir, name, true);
    skin.and(cape)
}

/// Moves `old`'s skin and cape to `new`'s files. Both names in the same letters are the same
/// files: nothing moves, and nothing is lost. A move that fails puts back what it moved.
pub fn rename(dir: &Path, old: &str, new: &str) -> Result<(), String> {
    let (from, to) = (files(dir, old), files(dir, new));
    let mut moved = Vec::new();
    for (from, to) in [(from.0, to.0), (from.1, to.1)] {
        if from == to || !from.is_file() {
            continue;
        }
        if let Err(e) = std::fs::rename(&from, &to) {
            for (from, to) in moved {
                let _ = std::fs::rename(to, from);
            }
            return Err(format!("{}: {e}", from.display()));
        }
        moved.push((from, to));
    }
    Ok(())
}

/// The variables the game gets for `name`'s look: its skin and model if it has a skin, its cape
/// if it has one.
pub fn env(dir: &Path, name: &str, model: Model) -> Vec<(&'static str, String)> {
    let (skin, cape) = files(dir, name);
    let mut out = Vec::new();
    if skin.is_file() {
        out.push((SKIN_ENV, skin.to_string_lossy().into_owned()));
        out.push((MODEL_ENV, model.name().to_string()));
    }
    if cape.is_file() {
        out.push((CAPE_ENV, cape.to_string_lossy().into_owned()));
    }
    out
}
