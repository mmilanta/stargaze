//! Runtime resources and native per-user storage, independent of the working directory.
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn configs() -> PathBuf {
    configs_for(std::env::current_exe().ok().as_deref())
}

fn configs_for(executable: Option<&Path>) -> PathBuf {
    // Release archives ship configs beside the executable. Cargo runs fall back
    // to the checkout; never require the release builder's checkout at runtime.
    if let Some(dir) = executable.and_then(Path::parent) {
        let bundled = dir.join("configs");
        if bundled.is_dir() {
            return bundled;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("configs")
}

pub fn progress() -> Option<PathBuf> {
    user_file(
        "STARGAZE_PROGRESS",
        dirs::data_local_dir(),
        "progress.yaml",
        "XDG_DATA_HOME",
        ".local/share",
    )
}

pub fn settings() -> Option<PathBuf> {
    user_file(
        "STARGAZE_SETTINGS",
        dirs::config_dir(),
        "graphics.yaml",
        "XDG_CONFIG_HOME",
        ".config",
    )
}

fn user_file(
    override_name: &str,
    native: Option<PathBuf>,
    filename: &str,
    legacy_variable: &str,
    legacy_suffix: &str,
) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(override_name) {
        return Some(path.into());
    }
    let target = native?.join("stargaze").join(filename);
    let legacy = std::env::var_os(legacy_variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(legacy_suffix)))
        .map(|p| p.join("stargaze").join(filename));
    Some(migrate(&target, legacy.as_deref()))
}

// Copy without removing the original or replacing an existing native file.
// If migration fails, keep using the legacy file rather than lose progress.
fn migrate(target: &Path, legacy: Option<&Path>) -> PathBuf {
    let Some(legacy) = legacy.filter(|p| *p != target && p.is_file()) else {
        return target.to_owned();
    };
    if target.exists() {
        return target.to_owned();
    }
    let result = (|| -> std::io::Result<()> {
        let bytes = std::fs::read(legacy)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)
        {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
            Err(err) => return Err(err),
        };
        if let Err(err) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(target);
            return Err(err);
        }
        Ok(())
    })();
    match result {
        Ok(()) => target.to_owned(),
        Err(err) => {
            log::warn!(
                "could not migrate {} to {}: {err}; using the original",
                legacy.display(),
                target.display()
            );
            legacy.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_configs_take_precedence_over_checkout() {
        let dir = std::env::temp_dir().join(format!("stargaze-resources-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("configs")).unwrap();
        assert_eq!(
            configs_for(Some(&dir.join("stargaze"))),
            dir.join("configs")
        );
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(
            configs_for(None),
            Path::new(env!("CARGO_MANIFEST_DIR")).join("configs")
        );
    }

    #[test]
    fn migration_preserves_original_and_never_overwrites_native_data() {
        let dir = std::env::temp_dir().join(format!("stargaze-migration-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("old.yaml");
        let new = dir.join("native/progress.yaml");
        std::fs::write(&old, "original").unwrap();
        assert_eq!(migrate(&new, Some(&old)), new);
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "original");
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "original");
        std::fs::write(&new, "new progress").unwrap();
        migrate(&new, Some(&old));
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "new progress");
        let blocked = dir.join("not-a-directory");
        std::fs::write(&blocked, "blocked").unwrap();
        assert_eq!(migrate(&blocked.join("save.yaml"), Some(&old)), old);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
