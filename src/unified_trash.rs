use crate::trashinfo::{self, Trashinfo};
use anyhow::Context;
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct UnifiedTrash {
    home_trash: Trash,
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
            true,
            false,
        )
        .context("Failed to get home trash dir")?;

        let real_uid = unsafe { libc::getuid() };
        let mut trashes =
            get_trash_dirs_from_mounts(real_uid).context("Failed to get trash dirs")?;
        trashes.insert(0, home_trash.clone());

        // ensure that admin created trash dirs take priority
        // yes b and a need to be swapped for this to be the proper way round
        trashes.sort_by(|a, b| b.is_admin_trash.cmp(&a.is_admin_trash));

        Ok(Self {
            trashes,
            home_trash,
        })
    }

    pub fn list(&self) -> anyhow::Result<Vec<Trashinfo>> {
        let mut parsed = vec![];
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info_dir()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), &trash.dev_root)
                    .context("Failed to parse dir entry")?;

                if !trash.files_dir().join(&info.trash_filename).exists() {
                    eprintln!(
                        "Warn: orphaned trashinfo: {}",
                        trash.files_dir().join(&info.trash_filename).display()
                    );
                    continue;
                }

                parsed.push(info);
            }
        }

        Ok(parsed)
    }

    pub fn put(&self, input_files: &[PathBuf]) -> anyhow::Result<()> {
        for input_file in input_files {
            let input_file_meta = fs::metadata(&input_file)
                .context(format!("Failed stat file: {}", input_file.display()))?;

            if is_sys_path(&input_file).context("Failed to determine if path is system path")? {
                eprintln!(
                    "Warn: trashing in system path {} is not supported.",
                    input_file.display()
                );
                continue;
            }

            let mut newfile_info = Trashinfo {
                trash_filename: input_file
                    .file_name()
                    .context("File has no filename")?
                    .into(),
                deleted_at: chrono::Local::now().naive_local(),
                original_filepath: input_file
                    .canonicalize()
                    .context("Failed to resolve path")?,
            };

            let trashed_files = self.list().context("Failed to list trash")?;

            for iterations in 1.. {
                if trashed_files
                    .iter()
                    .any(|x| x.trash_filename == newfile_info.trash_filename)
                {
                    let mut name_changed =
                        newfile_info.trash_filename.clone().as_os_str().to_owned();
                    name_changed.push(OsString::from(iterations.to_string()));

                    newfile_info.trash_filename.set_file_name(name_changed);
                    continue;
                } else {
                    break;
                }
            }

            // At this point we have a unique name

            if input_file_meta.dev() == self.home_trash.device {
                // input is on the same device as the home trash, so we use that.
                self.home_trash
                    .write(&newfile_info)
                    .context("Failed to write to home trash")?;
            } else {
                let existing_trash = self
                    .trashes
                    .iter()
                    .find(|x| x.device == input_file_meta.dev());

                if let Some(existing_trash) = existing_trash {
                    // We already have a trash on the device, so we use it
                    existing_trash
                        .write(&newfile_info)
                        .context("Failed to write to trash")?;
                } else {
                    // We don't have a trash on this device, so we create one
                    let mounts = list_mounts().context("Failed to list mounts")?;
                    let fs_root =
                        find_fs_root(&input_file).context("Failed to find mount point")?;

                    assert!(mounts.contains(&fs_root), "oh nein");

                    let fs_root_meta = fs::metadata(&fs_root).context("Failed to stat mount")?;
                    let trash_name = format!(".Trash-{}", unsafe { libc::getuid() });
                    let trash = Trash::new_with_ensure(
                        fs_root.join(trash_name),
                        fs_root.clone(),
                        fs_root_meta.dev(),
                        false,
                        false,
                    )
                    .context(format!(
                        "Failed to create trash dir on mount: {}",
                        &fs_root.display()
                    ))?;

                    trash
                        .write(&newfile_info)
                        .context("Failed writing to trash")?;
                }
            }

            println!("trashed {}", input_file.display());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Trash {
    is_home_trash: bool,
    is_admin_trash: bool,
    dev_root: PathBuf,
    trash_path: PathBuf,
    device: u64,
}

impl Trash {
    #[must_use]
    pub fn new_with_ensure(
        path: PathBuf,
        dev_root: PathBuf,
        device: u64,
        is_home_trash: bool,
        is_admin_trash: bool,
    ) -> anyhow::Result<Self> {
        fs::create_dir_all(path.join("files")).context("Failed to create files dir")?;
        fs::create_dir_all(path.join("info")).context("Failed to create info dir")?;

        Ok(Self {
            trash_path: path,
            device,
            dev_root,
            is_home_trash,
            is_admin_trash,
        })
    }

    #[must_use]
    fn write(&self, info: &Trashinfo) -> anyhow::Result<()> {
        let mut f = info
            .trash_filename
            .file_name()
            .context("Has no filename")?
            .to_os_string();
        f.push(".trashinfo");

        let full_infoname = self.info_dir().join(f);

        let mut info_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(full_infoname)
            .context("Failed to open info file")?;

        let trashinfo_file = if self.is_home_trash {
            info.trashinfo_file()
        } else {
            info.trashinfo_file_relative(&self.dev_root)
                .context("Failed to build relative path")?
        };

        info_file
            .write_all(trashinfo_file.as_bytes())
            .context("Failed to write to info file")?;

        match fs::rename(
            &info.original_filepath,
            self.files_dir().join(&info.trash_filename),
        )
        .context("Failed to move file")
        {
            Ok(v) => Ok(v),
            Err(e) => {
                eprintln!(
                    "Error: Failed moving file {}, reverting info file...",
                    info.original_filepath.display()
                );
                fs::remove_file(
                    self.info_dir()
                        .join(&info.trash_filename)
                        .with_extension("trashinfo"),
                )
                .context("Failed to remove existing info file")?;

                Err(e)
            }
        }
    }

    pub fn files_dir(&self) -> PathBuf {
        self.trash_path.join("files")
    }

    pub fn info_dir(&self) -> PathBuf {
        self.trash_path.join("info")
    }
}

/// Panics if /proc/mounts has unexpected format.
#[must_use]
fn get_trash_dirs_from_mounts(uid: u32) -> anyhow::Result<Vec<Trash>> {
    let top_dirs = list_mounts().context("Failed to list mounts")?;

    let mut trash_dirs = vec![];
    for top_dir in top_dirs {
        // what the spec calls $top_dir/.Trash
        let admin_dir = top_dir.join(".Trash");

        // the admin dir exists
        if let Ok(admin_dir_meta) = fs::metadata(&admin_dir) {
            let mut checks_passed = false;
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
                            false,
                            true,
                        );
                        if let Ok(new_trash) = new_trash {
                            trash_dirs.push(new_trash);
                            checks_passed = true;
                            // we intentionally don't `continue` here, since both admin and uid
                            // trash dirs should be supported at once.
                        }
                    }
                }
            }

            if !checks_passed {
                eprintln!(
                    "Warn: {} does not pass checks, ignoring",
                    admin_dir.display()
                )
            }
        };

        // we continue with $top_dir/.Trash-$uid or, as we will call it, the uid_dir

        let uid_dir = top_dir.join(format!(".Trash-{uid}"));

        if let Ok(uid_dir_meta) = fs::metadata(&uid_dir) {
            if let Ok(new_trash) =
                Trash::new_with_ensure(uid_dir, top_dir, uid_dir_meta.dev(), false, false)
            {
                trash_dirs.push(new_trash);
            }
        }
    }

    Ok(trash_dirs)
}

#[must_use]
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

#[must_use]
fn is_sys_path(path: &Path) -> anyhow::Result<bool> {
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
fn find_fs_root(path: &Path) -> anyhow::Result<PathBuf> {
    let path = path.canonicalize().context("Failed to resolve path")?;
    let root_dev = fs::metadata(&path).context("Failed to get metadata")?.dev();
    Ok(path
        .ancestors() // trust the metadata call won't fail
        .take_while(|x| fs::metadata(x).unwrap().dev() == root_dev)
        .collect())
}
