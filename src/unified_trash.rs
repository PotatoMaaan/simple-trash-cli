use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

use anyhow::Context;

use crate::trashinfo::{self, Trashinfo};

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
        )
        .context("Failed to get home trash dir")?;

        let real_uid = unsafe { libc::getuid() };
        let mut trashes =
            get_trash_dirs_from_mounts(real_uid).context("Failed to get trash dirs")?;
        trashes.insert(0, home_trash.clone());

        Ok(Self {
            trashes,
            home_trash,
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

    pub fn put(&self, input_files: &[PathBuf]) -> anyhow::Result<()> {
        for input_file in input_files {
            let input_file_meta = fs::metadata(&input_file)
                .context(format!("Failed stat file: {}", input_file.display()))?;

            if is_sys_path(&input_file).context("Failed to determine if path is system path")? {
                eprintln!(
                    "Warn: trashing in system path {} is not supported.",
                    input_file.display()
                );
            }

            let mut new_info = Trashinfo {
                trash_filename: input_file
                    .file_name()
                    .context("File has no filename")?
                    .into(),
                deleted_at: chrono::Local::now().naive_local(),
                original_filepath: input_file
                    .canonicalize()
                    .context("Failed to resolve path")?,
            };

            // We continue appending the current number of iterations to the filename until
            // such a file no longer exists in any of the trash locations.
            let mut iters = 0;
            let mut successful_locks = loop {
                iters += 1;

                let attempted_writes = self
                    .trashes
                    .iter()
                    .map(|trash| trash.try_lock_file(&new_info))
                    .collect::<Vec<_>>();

                if attempted_writes.iter().any(already_exists) {
                    let mut name_changed = new_info.trash_filename.as_os_str().to_owned();
                    name_changed.push(OsString::from(iters.to_string()));

                    new_info.trash_filename.set_file_name(name_changed);
                    continue;
                } else {
                    break attempted_writes
                        .into_iter()
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>();
                }
            };

            // The input is on the same device as the home trash, so that one takes priority
            if input_file_meta.dev() == self.home_trash.device {
                let home_trash_index = successful_locks
                    .iter()
                    .position(|info| info.trash == self.home_trash)
                    .context("File on same device as home trash dir, but the home trash could not be opened")?;

                let locked = successful_locks.remove(home_trash_index);

                write_trash(locked, input_file).context("Failed to write to trash")?;
                println!("trashed {}", input_file.display());
                continue;
            }

            // At this point the home trash is already handled and we are on a different device

            let device_trash = if let Some(idx) = successful_locks
                .iter()
                .position(|lock| lock.trash.device == input_file_meta.dev())
            {
                successful_locks.remove(idx)
            } else {
                // A trash dir does not exist on the mount, so we try to create one.

                let mounts = list_mounts().context("Failed to list mounts")?;
                let fs_root = find_fs_root(&input_file).context("Failed to find mount point")?;

                assert!(mounts.contains(&fs_root), "oh nein");

                let fs_root_meta = fs::metadata(&fs_root).context("Failed to stat mount")?;
                let trash_name = format!(".Trash-{}", unsafe { libc::getuid() });
                let trash = Trash::new_with_ensure(
                    fs_root.join(trash_name),
                    fs_root.clone(),
                    fs_root_meta.dev(),
                )
                .context(format!(
                    "Failed to create trash dir on moun: {}",
                    &fs_root.display()
                ))?;

                trash
                    .try_lock_file(&new_info)
                    .context("Failed to lock file")?
            };

            write_trash(device_trash, &input_file).context("Failed to write to trash")?;
            println!("trashed {}", input_file.display());
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Trash {
    dev_root: PathBuf,
    trash_path: PathBuf,
    device: u64,
}

fn already_exists(res: &io::Result<LockedTrashinfo>) -> bool {
    match res {
        Ok(_) => false,
        Err(e) => match e.kind() {
            io::ErrorKind::AlreadyExists => true,
            _ => false,
        },
    }
}

#[derive(Debug)]
struct LockedTrashinfo {
    handle: File,
    info: Trashinfo,
    trash: Trash,
}

fn write_trash(mut lock: LockedTrashinfo, move_file: &Path) -> anyhow::Result<()> {
    let info_file = lock.info.to_trashinfo_file();
    lock.handle
        .write(info_file.as_bytes())
        .context("Failed to write info file")?;

    match fs::rename(
        move_file,
        lock.trash.files().join(&lock.info.trash_filename),
    )
    .context("Failed to move file to trash")
    {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Err: Failed to move file to trash: {}", e);
            eprintln!("Attempting to remove info file...");
            fs::remove_file(lock.trash.info().join(&lock.info.trash_filename))
                .expect("Failed to remove just created info file");

            Err(e)
        }
    }
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

    pub fn try_lock_file(&self, info: &Trashinfo) -> io::Result<LockedTrashinfo> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(
                self.info()
                    .join(info.trash_filename.with_extension("trashinfo")),
            )
            .map(|x| LockedTrashinfo {
                handle: x,
                info: info.clone(),
                trash: self.clone(),
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

fn is_sys_path(path: &Path) -> anyhow::Result<bool> {
    let path = path.canonicalize().context("Failed to resolve path")?;

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

fn find_fs_root(path: &Path) -> anyhow::Result<PathBuf> {
    let path = path.canonicalize().context("Failed to resolve path")?;
    let root_dev = fs::metadata(&path).context("Failed to get metadata")?.dev();
    Ok(path
        .ancestors() // trust the metadata call won't fail
        .take_while(|x| fs::metadata(x).unwrap().dev() == root_dev)
        .collect())
}
