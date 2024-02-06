use anyhow::Context;
use format as f;
use log::warn;
use std::{
    ffi::{OsStr, OsString},
    fs::{self},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
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

    /// Removes any orphaned trashinfo files, i.e `.trashinfo` files that don't have a
    /// matching file actually *in* the trash
    pub fn remove_orphaned(&self) -> anyhow::Result<()> {
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info_dir()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), &trash.dev_root)
                    .context("Failed to parse dir entry")?;

                if !trash.files_dir().join(&info.trash_filename).exists() {
                    let info_file = trash
                        .info_dir()
                        .join(&info.trash_filename_trashinfo)
                        .with_extension("trashinfo");

                    log::info!("Removing orphaned trashinfo file: {}", info_file.display());

                    fs::remove_file(&info_file).context("Failed to remove info file")?;
                    continue;
                }
            }
        }

        Ok(())
    }

    /// List all currently trashed files.
    ///
    /// Note that is is according to the `.trashinfo` files, i.e a file without the
    /// matching `.trashinfo` file is *not* listed, as not enough information
    /// can be gathered to fully construct a `Trashinfo` object.
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
                            .join(&info.trash_filename_trashinfo)
                            .display()
                    );
                    continue;
                }

                parsed.push((trash, info));
            }
        }

        Ok(parsed)
    }

    /// Attempts to trash the `input_file`, creating a new trashcan on the device if needed.
    pub fn put(&self, input_file: &Path) -> anyhow::Result<()> {
        let input_file_meta = fs::metadata(&input_file)
            .context(format!("Failed stat file: {}", input_file.display()))?;

        if is_sys_path(&input_file) {
            anyhow::bail!(
                "Trashing in system path {} is not supported",
                input_file.display()
            );
        }

        let trash_filename = input_file
            .file_name()
            .context("File has no filename")?
            .to_os_string();

        let mut trash_filename_trashinfo = trash_filename.clone();
        trash_filename_trashinfo.push(OsString::from(".trashinfo"));

        // the trashinfo for the new file, this gets updated if the file already exists
        let mut newfile_info = Trashinfo {
            trash_filename: trash_filename.clone(),
            trash_filename_trashinfo,
            deleted_at: chrono::Local::now().naive_local(),
            original_filepath: input_file
                .canonicalize()
                .context("Failed to resolve path")?,
        };

        // by listing all trashes, we ensure that the filename is unique system wide,
        // as far as i can tell, this is what nautilus does as well and genereally seems like a good idea
        let trashed_files = self.list().context("Failed to list trash")?;

        for iterations in 1.. {
            if trashed_files
                .iter()
                .any(|(_, x)| x.trash_filename == newfile_info.trash_filename)
            {
                // If we get here, a file with the current name already exists in one of the trashes,
                // so we append the current iteration number to it and check again
                // we try to preserve the extension in case a user wants to manually recover a file
                // (so it still has the proper extension)

                // somefile.txt
                let old_name = PathBuf::from(&trash_filename);

                // somefile
                let mut stem = old_name
                    .file_stem()
                    .unwrap_or(&newfile_info.trash_filename)
                    .to_os_string();

                // txt
                let ext = old_name.extension();

                // somefile1
                stem.push(OsStr::new(&iterations.to_string()));

                if let Some(ext) = ext {
                    // somefile1.txt
                    stem.push(OsStr::new("."));
                    stem.push(ext);
                }

                newfile_info.rename(stem);

                continue;
            } else {
                // we have a unique filename
                break;
            }
        }

        // At this point we have a unique name

        if input_file_meta.dev() == self.home_trash.device {
            // input is on the same device as the home trash, so we use that.
            self.home_trash
                .write_trashinfo(&newfile_info)
                .context("Failed to write to home trash")?;
        } else {
            let existing_trash = self
                .trashes
                .iter()
                .find(|x| x.device == input_file_meta.dev());

            if let Some(existing_trash) = existing_trash {
                // We already have a trash on the device, so we use it
                existing_trash
                    .write_trashinfo(&newfile_info)
                    .context("Failed to write to trash")?;
            } else {
                // We don't have a trash on this device, so we create one
                let mounts = list_mounts().context("Failed to list mounts")?;
                let fs_root = find_fs_root(&input_file).context("Failed to find mount point")?;

                assert!(mounts.contains(&fs_root), "oh nein");

                let fs_root_meta = fs::metadata(&fs_root).context("Failed to stat mount")?;
                let uid = unsafe { libc::getuid() };
                let trash_name = format!(".Trash-{}", uid);
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
                    .write_trashinfo(&newfile_info)
                    .context("Failed writing to trash")?;
            }
        }

        Ok(())
    }

    /// Empty the trash based on the `.trashinfo` files, meaning that files for which no
    /// `.trashinfo` file exists will be ignored
    pub fn empty(&self, before: chrono::NaiveDateTime, dry_run: bool) -> anyhow::Result<()> {
        for (trash, info) in self.list().context("Failed to list trash files")? {
            if info.deleted_at < before {
                let files_file = trash.files_dir().join(info.trash_filename);
                let info_file = trash.info_dir().join(info.trash_filename_trashinfo);

                if dry_run {
                    println!("Would delete {}", info.original_filepath.display());
                    continue;
                }

                let remove_result = if files_file.is_file() {
                    fs::remove_file(&files_file)
                } else {
                    fs::remove_dir_all(&files_file)
                };

                if let Err(e) = remove_result {
                    match e.kind() {
                        std::io::ErrorKind::NotFound => {
                            log::info!("Removing orphaned trashinfo file {}", info_file.display());
                            // This falls through to the remove_file call below
                        }
                        _ => {
                            anyhow::bail!(f!(
                                "Failed to remove file {}: {}",
                                files_file.display(),
                                e
                            ));
                        }
                    }
                }

                fs::remove_file(&info_file)
                    .context(f!("Failed to remove info file {}", info_file.display()))?;
            }
        }

        Ok(())
    }

    pub fn restore(
        &self,
        path: &Path,
        exists_callback: impl for<'a> Fn(&'a [(&Trash, Trashinfo)]) -> &'a Trashinfo,
    ) -> anyhow::Result<()> {
        let trashed_files = self.list().context("Failed to list trashed files")?;
        let matching = trashed_files
            .into_iter()
            .filter(|(_, x)| x.original_filepath == path)
            .collect::<Vec<_>>();

        match matching.len() {
            0 => anyhow::bail!("No files match"),
            1 => restore_file(&matching[0].1)?,
            _ => {
                let del = exists_callback(&matching);
            }
        }

        fn restore_file(p: &Trashinfo) -> anyhow::Result<()> {
            Ok(())
        }

        todo!()
    }
}
