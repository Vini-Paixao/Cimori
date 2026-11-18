//! Cimori's folder under %APPDATA% (settings, log, icon packs, stores) and %LOCALAPPDATA% (caches, assistant).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The folder's name under each base.
pub const NAME: &str = "Cimori";
/// Its name before the public release.
const OLD_NAME: &str = "isle-win";

/// The old `isle-win` folder becomes `Cimori` when there is none yet: renamed, copied if that fails.
/// The old folder when it was moved.
pub fn migrate(base: &Path) -> io::Result<Option<PathBuf>> {
    let (old, new) = (base.join(OLD_NAME), base.join(NAME));
    if new.exists() || !old.is_dir() {
        return Ok(None);
    }
    if fs::rename(&old, &new).is_err() {
        copy_dir(&old, &new)?;
        let _ = fs::remove_dir_all(&old);
    }
    Ok(Some(old))
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() { copy_dir(&entry.path(), &target)? } else { fs::copy(entry.path(), target)?; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_folder_moves_once() {
        let base = std::env::temp_dir().join(format!("cimori-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(base.join("isle-win/icons")).unwrap();
        fs::write(base.join("isle-win/settings.json"), "{}").unwrap();
        fs::write(base.join("isle-win/icons/a.png"), "x").unwrap();
        assert_eq!(migrate(&base).unwrap(), Some(base.join("isle-win")));
        assert!(!base.join("isle-win").exists());
        assert_eq!(fs::read_to_string(base.join("Cimori/settings.json")).unwrap(), "{}");
        assert!(base.join("Cimori/icons/a.png").is_file());
        // Cimori's own folder is never replaced by an old one.
        fs::create_dir_all(base.join("isle-win")).unwrap();
        assert_eq!(migrate(&base).unwrap(), None);
        assert!(base.join("isle-win").is_dir());
        // A copy keeps the same files.
        copy_dir(&base.join("Cimori"), &base.join("copy")).unwrap();
        assert!(base.join("copy/icons/a.png").is_file());
        fs::remove_dir_all(&base).unwrap();
    }
}
