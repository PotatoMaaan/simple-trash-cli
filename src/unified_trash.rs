use std::{
    env,
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
    home_trash_dev: u64,
    trashes: Vec<Trash>,
}

impl UnifiedTrash {
    pub fn new() -> anyhow::Result<Self> {
        let home_dir = PathBuf::from(env::var("HOME").context("No home dir set!")?);
        let xdg_data_dir = env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or(home_dir.join(".local").join("share"));
        let xdg_data_dir_meta = fs::metadata(&xdg_data_dir).context("Failed to get metadata")?;
        let home_trash = Trash::new_with_ensure(
            xdg_data_dir.join("Trash"),
            xdg_data_dir,
            xdg_data_dir_meta.dev(),
        )
        .context("Failed to get home trash dir")?;

        let real_uid = unsafe { libc::getuid() };
        let mut trashes =
            get_trash_dirs_from_mounts(real_uid).context("Failed to get trash dirs")?;
        trashes.insert(0, home_trash);

        Ok(Self {
            trashes,
            home_trash_dev: xdg_data_dir_meta.dev(),
        })
    }

    pub fn list(&self) -> anyhow::Result<Vec<Trashinfo>> {
        let mut parsed = vec![];
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), &trash.dev_root)
                    .context("Failed to parse dir entry")?;

                if !trash.files().join(&info.trash_filename).exists() {
                    eprintln!(
                        "Warn: orphaned trashinfo: {}",
                        trash.files().join(&info.trash_filename).display()
                    )
                }
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
    use super::UnifiedTrash;
    use std::{path::PathBuf, process::Command};

    #[test]
    fn me_when() {
        let trash = UnifiedTrash::new().unwrap();

        let gio_output = Command::new("gio")
            .arg("trash")
            .arg("--list")
            .output()
            .unwrap()
            .stdout;
        let gio_output = String::from_utf8(gio_output).unwrap();
        let mut gio_output = gio_output
            .lines()
            .map(|x| x.split("\t").skip(1).next().unwrap())
            .map(PathBuf::from)
            .collect::<Vec<_>>();

        let mut our_output = trash
            .list()
            .unwrap()
            .into_iter()
            .map(|x| x.original_filepath)
            .collect::<Vec<_>>();

        our_output.sort();
        gio_output.sort();

        assert_eq!(our_output, gio_output);
    }
}
