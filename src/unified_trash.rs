use std::{
    ffi::{OsStr, OsString},
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::trashinfo::{self, Trashinfo};

#[derive(Debug)]
pub struct UnifiedTrash {
    trashes: Vec<Trash>,
}

impl UnifiedTrash {
    pub fn new() -> anyhow::Result<Self> {
        let real_uid = unsafe { libc::getuid() };
        let mount_trashes =
            get_trash_dirs_from_mounts(real_uid).context("Failed to get trash dirs")?;

        Ok(Self {
            trashes: mount_trashes,
        })
    }

    pub fn list(&self) -> anyhow::Result<Vec<Trashinfo>> {
        let mut parsed = vec![];
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), &trash.dev_root)
                    .context("Failed to parse dir entry")?;
                parsed.push(info);
            }
        }

        Ok(parsed)
    }
}

#[derive(Debug)]
struct Trash {
    dev_root: PathBuf,
    trash_path: PathBuf,
    device: u64,
}

impl Trash {
    pub fn new_with_ensure(path: PathBuf, dev_root: PathBuf, device: u64) -> anyhow::Result<Self> {
        fs::create_dir_all(path.join("files")).context("Failed to create files dir")?;
        fs::create_dir_all(path.join("info")).context("Failed to create info dir")?;

        Ok(Self {
            trash_path: path,
            device,
            dev_root,
        })
    }

    pub fn files(&self) -> PathBuf {
        self.trash_path.join("files")
    }

    pub fn info(&self) -> PathBuf {
        self.trash_path.join("info")
    }
}

fn build_uid_trash_name(real_uid: u32) -> OsString {
    format!(".Trash-{}", real_uid).into()
}

/// Panics if /proc/mounts has unexpected format.
fn get_trash_dirs_from_mounts(uid: u32) -> anyhow::Result<Vec<Trash>> {
    let top_dirs = list_mounts().context("Failed to list mounts")?;

    let mut trash_dirs = vec![];
    for top_dir in top_dirs {
        // what the spec calls $top_dir/.Trash
        let admin_dir = top_dir.join(".Trash");

        // the admin dir exists
        if let Ok(admin_dir_meta) = fs::metadata(&admin_dir) {
            // the sticky bit is set (required according to spec)
            if admin_dir_meta.permissions().mode() & 0o1000 != 0 {
                // the admin dir is not a symlink (also required)
                if !admin_dir_meta.is_symlink() {
                    let admin_uid_dir = admin_dir.join(uid.to_string());

                    // ensure $top_dir/.Trash/$uid exists
                    if fs::create_dir_all(&admin_uid_dir).is_ok() {
                        // ensure $top_dir/.Trash/$uid/files and $top_dir/.Trash/$uid/info exist
                        let new_trash = Trash::new_with_ensure(
                            admin_uid_dir,
                            top_dir.clone(),
                            admin_dir_meta.dev(),
                        );
                        if let Ok(new_trash) = new_trash {
                            trash_dirs.push(new_trash);
                            continue;
                        }
                    }
                }
            }

            eprintln!(
                "Warn: {} does not pass checks, ignoring",
                admin_dir.display()
            )
        };

        // At this point the admin dir does not exist or failed the checks
        // so we continue with $top_dir/.Trash-$uid or, as we will call it, the uid_dir

        let uid_dir = top_dir.join(format!(".Trash-{uid}"));

        if let Ok(uid_dir_meta) = fs::metadata(&uid_dir) {
            if let Ok(new_trash) = Trash::new_with_ensure(uid_dir, top_dir, uid_dir_meta.dev()) {
                trash_dirs.push(new_trash);
            }
        }
    }

    Ok(trash_dirs)
}

fn list_mounts() -> Result<Vec<PathBuf>, anyhow::Error> {
    Ok(fs::read("/proc/mounts")
        .context("Failed to read /proc/mounts, are you perhaps not running linux?")?
        .split(|x| *x as char == '\n')
        .filter(|x| !x.is_empty())
        .map(|x| x.split(|x| *x == (' ' as u8)).skip(1).next().unwrap())
        .map(OsStr::from_bytes)
        .map(PathBuf::from)
        .collect())
}

#[cfg(test)]
mod test {

    use super::{get_trash_dirs_from_mounts, UnifiedTrash};

    #[test]
    fn me_when() {
        let trash = UnifiedTrash::new().unwrap();
        for p in trash.list().unwrap() {
            println!(
                "{}\t{}",
                p.trash_filename.display(),
                p.original_filepath.display()
            );
        }
    }
}
