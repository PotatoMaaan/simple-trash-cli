use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};

use anyhow::Context;

use super::{list_mounts, trashinfo::Trashinfo};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trash {
    pub is_home_trash: bool,
    pub is_admin_trash: bool,
    pub dev_root: PathBuf,
    pub trash_path: PathBuf,
    pub device: u64,
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
    pub fn write(&self, info: &Trashinfo) -> anyhow::Result<()> {
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

    /// Panics if /proc/mounts has unexpected format.
    #[must_use]
    pub fn get_trash_dirs_from_mounts(uid: u32) -> anyhow::Result<Vec<Trash>> {
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
}