//! Versioned, atomic per-level saves. A failed write leaves the last save intact.
use crate::game::{Game, Snapshot};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub theory: Snapshot,
    pub time: f64,
    pub az: f64,
    pub alt: f64,
    pub fov: f64,
    pub orientation: Option<[[f64; 3]; 2]>,
    pub lock: Option<SavedLock>,
    pub ev: f32,
    pub auto: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SavedLock {
    Body(usize),
    Sky([f64; 3]),
}

impl Session {
    pub fn valid(&self) -> bool {
        Game::restore(&self.theory).is_some()
            && [self.time, self.az, self.alt, self.fov]
                .iter()
                .all(|v| v.is_finite())
            && self.alt.abs() <= std::f64::consts::FRAC_PI_2 + 0.001
            && (0.001_f64.to_radians()..=90.0_f64.to_radians()).contains(&self.fov)
            && self.ev.is_finite()
            && (-8.0..=8.0).contains(&self.ev)
            && self.orientation.is_none_or(|vectors| {
                vectors.iter().all(|v| {
                    v.iter().all(|x| x.is_finite()) && v.iter().map(|x| x * x).sum::<f64>() > 0.5
                })
            })
            && self.lock.as_ref().is_none_or(|lock| match lock {
                SavedLock::Body(_) => true,
                SavedLock::Sky(v) => {
                    v.iter().all(|x| x.is_finite()) && v.iter().map(|x| x * x).sum::<f64>() > 0.5
                }
            })
    }
    pub fn summary(&self) -> (usize, usize, Option<f32>) {
        Game::restore(&self.theory)
            .map(|g| g.progress())
            .unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    levels: BTreeMap<String, Session>,
    favorites: BTreeSet<String>,
}

#[derive(Default)]
pub struct Book {
    pub levels: BTreeMap<String, Session>,
    pub favorites: BTreeSet<String>,
    pub message: Option<String>,
    path: Option<PathBuf>,
}

pub fn key(path: &Path) -> String {
    // Bundled levels keep stable portable keys; external levels use full paths.
    let configs = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/configs"));
    let absolute = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    absolute
        .strip_prefix(configs)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| absolute.to_string_lossy().into_owned())
}

fn default_path() -> Option<PathBuf> {
    std::env::var_os("STARGAZE_PROGRESS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
                .map(|p| p.join("stargaze/progress.yaml"))
        })
}

impl Book {
    pub fn load() -> Self {
        Self::load_from(default_path())
    }
    pub fn load_from(path: Option<PathBuf>) -> Self {
        let mut book = Self {
            path,
            ..Self::default()
        };
        if let Some(path) = &book.path {
            match std::fs::read_to_string(path)
                .context("reading progress")
                .and_then(|s| serde_saphyr::from_str::<File>(&s).context("parsing progress"))
            {
                Ok(mut file) if file.version == 1 => {
                    let before = file.levels.len();
                    file.levels.retain(|_, session| session.valid());
                    if before != file.levels.len() {
                        book.message = Some(
                            "Some saved levels could not be read. Other progress was restored."
                                .into(),
                        );
                    }
                    book.levels = file.levels;
                    book.favorites = file.favorites;
                }
                Ok(_) => {
                    book.message = Some(
                        "This progress file uses an unsupported version. It has been preserved."
                            .into(),
                    )
                }
                Err(err) if path.exists() => {
                    log::warn!("could not load progress: {err:#}");
                    book.message = Some(
                        "Saved progress could not be read. The original file has been preserved."
                            .into(),
                    );
                }
                Err(_) => {}
            }
            // Preserve unreadable/unknown versions instead of overwriting them.
            if book.message.is_some() && book.levels.is_empty() {
                book.path = None;
            }
        }
        book
    }
    pub fn persist(&mut self) -> Result<()> {
        let path = self
            .path
            .as_ref()
            .context("Progress is available for this session; no writable save path.")?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let file = File {
            version: 1,
            levels: self.levels.clone(),
            favorites: self.favorites.clone(),
        };
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_saphyr::to_string(&file)?)?;
        std::fs::rename(&temporary, path).context("saving puzzle progress")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_levels_favorites_and_invalid_save_preservation() {
        let dir =
            std::env::temp_dir().join(format!("stargaze-progress-test-{}", std::process::id()));
        let path = dir.join("progress.yaml");
        let mut book = Book::load_from(Some(path.clone()));
        book.levels.insert(
            "puzzle.yaml".into(),
            Session {
                theory: Game::default().snapshot(),
                time: 4.0,
                az: 1.0,
                alt: 0.4,
                fov: 0.7,
                orientation: None,
                lock: None,
                ev: 0.0,
                auto: true,
            },
        );
        book.favorites.insert("halo.yaml".into());
        book.persist().unwrap();
        let loaded = Book::load_from(Some(path.clone()));
        assert_eq!(loaded.levels["puzzle.yaml"].time, 4.0);
        assert!(loaded.favorites.contains("halo.yaml"));
        std::fs::write(&path, "invalid: [").unwrap();
        let mut damaged = Book::load_from(Some(path.clone()));
        assert!(damaged.message.is_some());
        assert!(damaged.persist().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid: [");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
