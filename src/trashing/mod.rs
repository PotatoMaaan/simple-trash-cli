use anyhow::Context;
use std::{
    env,
    ffi::OsStr,
    fs,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Path, PathBuf},
};

mod trash;
mod trashinfo;
mod unified_trash;

pub use trash::Trash;
pub use trashinfo::Trashinfo;
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
pub fn is_sys_path(path: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };

    if path == PathBuf::from("/") {
        return true;
    }

    let Some(first_component) = path.components().skip(1).next() else {
        return false;
    };
    let first_component = first_component.as_os_str();

    match first_component.to_string_lossy().to_string().as_str() {
        "boot" => true,
        "dev" => true,
        "proc" => true,
        "lost+found" => true,
        "sys" => true,
        "tmp" => true,
        _ => false,
    }
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

#[must_use]
pub fn find_home_trash() -> anyhow::Result<Trash> {
    let home_dir = PathBuf::from(env::var("HOME").context("No home dir set!")?);
    let xdg_data_dir = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home_dir.join(".local").join("share"));
    let xdg_data_dir_meta = fs::metadata(&xdg_data_dir).context("Failed to get metadata")?;
    Trash::new_with_ensure(
        xdg_data_dir.join("Trash"),
        xdg_data_dir,
        xdg_data_dir_meta.dev(),
        true,
        false,
    )
}

#[test]
fn test_is_sys_path1() {
    let p = PathBuf::from("/dev/usb");
    assert_eq!(is_sys_path(&p), true);
}

#[test]
fn test_is_sys_path2() {
    let p = PathBuf::from("/proc/mounts");
    assert_eq!(is_sys_path(&p), true);
}

#[test]
fn test_is_sys_path3() {
    let p = PathBuf::from("/home");

    assert_eq!(is_sys_path(&p), false);
}

#[test]
fn test_is_sys_path4() {
    let p = PathBuf::from("/");

    assert_eq!(is_sys_path(&p), true);
}
