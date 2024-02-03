use self::trash::Trash;
use std::{
    ffi::OsStr,
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

mod trash;
mod trashinfo;
mod unified_trash;

use anyhow::Context;
pub use unified_trash::UnifiedTrash;

#[must_use]
pub fn list_mounts() -> Result<Vec<PathBuf>, anyhow::Error> {
    Ok(fs::read("/proc/mounts")
        .context("Failed to read /proc/mounts, are you perhaps not running linux?")?
        .split(|x| *x as char == '\n')
        .filter(|x| !x.is_empty())
        .map(|x| x.split(|x| *x == (' ' as u8)).skip(1).next().unwrap())
        .map(OsStr::from_bytes)
        .map(PathBuf::from)
        .collect())
}

#[must_use]
pub fn is_sys_path(path: &Path) -> anyhow::Result<bool> {
    let path = path.canonicalize().context("Failed to resolve path")?;

    if path == PathBuf::from("/") {
        return Ok(true);
    }

    let first_component = path
        .components()
        .next()
        .context("Path has no first element")?
        .as_os_str();

    Ok(
        match first_component.to_string_lossy().to_string().as_str() {
            "boot" => true,
            "dev" => true,
            "proc" => true,
            "lost+found" => true,
            "sys" => true,
            "tmp" => true,
            _ => false,
        },
    )
}

#[must_use]
pub fn find_fs_root(path: &Path) -> anyhow::Result<PathBuf> {
    let path = path.canonicalize().context("Failed to resolve path")?;
    let root_dev = fs::metadata(&path).context("Failed to get metadata")?.dev();
    Ok(path
        .ancestors() // trust the metadata call won't fail
        .take_while(|x| fs::metadata(x).unwrap().dev() == root_dev)
        .collect())
}
