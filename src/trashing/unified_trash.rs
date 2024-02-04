use anyhow::Context;
use log::warn;
use std::{
    ffi::OsString,
    fs::{self},
    os::unix::fs::MetadataExt,
    path::PathBuf,
};

use crate::trashing::{find_fs_root, is_sys_path, list_mounts};

use super::{
    find_home_trash,
    trash::Trash,
    trashinfo::{self, Trashinfo},
};

#[derive(Debug)]
/// Provides a wrapper around all trashcans across all pysical devices.
pub struct UnifiedTrash {
    home_trash: Trash,
    trashes: Vec<Trash>,
}

impl UnifiedTrash {
    pub fn new() -> anyhow::Result<Self> {
        let home_trash = find_home_trash().context("Failed to get home trash dir")?;

        let real_uid = unsafe { libc::getuid() };
        let mut trashes =
            Trash::get_trash_dirs_from_mounts(real_uid).context("Failed to get trash dirs")?;
        trashes.insert(0, home_trash.clone());

        // ensure that admin created trash dirs take priority.
        // yes a and b need to be swapped for this to be the proper way round
        trashes.sort_by(|a, b| b.is_admin_trash.cmp(&a.is_admin_trash));

        Ok(Self {
            trashes,
            home_trash,
        })
    }

    pub fn list(&self) -> anyhow::Result<Vec<(&Trash, Trashinfo)>> {
        let mut parsed = vec![];
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info_dir()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), &trash.dev_root)
                    .context("Failed to parse dir entry")?;

                if !trash.files_dir().join(&info.trash_filename).exists() {
                    warn!(
                        "Orphaned trashinfo file: {}",
                        trash
                            .files_dir()
                            .join(&info.trash_filename)
                            .with_extension("trashinfo")
                            .display()
                    );
                    continue;
                }

                parsed.push((trash, info));
            }
        }

        Ok(parsed)
    }

    pub fn put(&self, input_files: &[PathBuf]) -> anyhow::Result<()> {
        for input_file in input_files {
            let input_file_meta = fs::metadata(&input_file)
                .context(format!("Failed stat file: {}", input_file.display()))?;

            if is_sys_path(&input_file) {
                warn!(
                    "trashing in system path {} is not supported.",
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
                    .any(|(_, x)| x.trash_filename == newfile_info.trash_filename)
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
