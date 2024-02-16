use anyhow::Context;
use format as f;
use log::warn;
use std::{
    ffi::{OsStr, OsString},
    fs::{self},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::trashing::{find_fs_root, is_sys_path};

use super::{
    find_home_trash, lexical_absolute,
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

    pub fn list_trashes(&self) -> &[Trash] {
        &self.trashes
    }

    /// Removes any orphaned trashinfo files, i.e `.trashinfo` files that don't have a
    /// matching file actually *in* the trash
    pub fn remove_orphaned(&self) -> anyhow::Result<()> {
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info_dir()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                let info = trashinfo::parse_trashinfo(&info.path(), trash)
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
    pub fn list(&self) -> anyhow::Result<Vec<Trashinfo>> {
        let mut parsed = vec![];
        for trash in &self.trashes {
            for info in fs::read_dir(trash.info_dir()).context("Failed to read info dir")? {
                let info = info.context("Failed to get dir entry")?;
                log::trace!("Parsing {}", info.path().display());
                let info = trashinfo::parse_trashinfo(&info.path(), trash)
                    .context("Failed to parse dir entry")?;

                let files_path = trash.files_dir().join(&info.trash_filename);

                match fs::symlink_metadata(&files_path) {
                    Ok(v) => v,
                    Err(e) => match e.kind() {
                        std::io::ErrorKind::NotFound => {
                            warn!(
                                "Orphaned trashinfo file: {}",
                                trash
                                    .info_dir()
                                    .join(&info.trash_filename_trashinfo)
                                    .display()
                            );
                            continue;
                        }
                        _ => anyhow::bail!("Failed to stat {}", files_path.display()),
                    },
                };

                parsed.push(info);
            }
        }

        Ok(parsed)
    }

    /// Attempts to trash the `input_file`, creating a new trashcan on the device if needed.
    pub fn put(&self, input_file: &Path, follow_links: bool) -> anyhow::Result<()> {
        let deleted_at = chrono::Local::now().naive_local();

        let input_file_meta = fs::symlink_metadata(input_file)
            .context(format!("Failed stat file: {}", input_file.display()))?;

        let original_filepath = if follow_links {
            input_file
                .canonicalize()
                .context("Failed to resolve path path")?
        } else {
            lexical_absolute(input_file).context("Failed to build lexical absolute path")?
        };

        if is_sys_path(input_file) {
            anyhow::bail!(
                "Trashing in system path {} is not supported",
                input_file.display()
            );
        }

        let mut new_file_name = input_file
            .file_name()
            .context("File has no filename")?
            .to_os_string();

        // by listing all trashes, we ensure that the filename is unique system wide,
        // as far as i can tell, this is what nautilus does as well and genereally seems like a good idea
        let trashed_files = self.list().context("Failed to list trash")?;

        {
            let orig_filename = new_file_name.clone();

            for iterations in 1.. {
                if trashed_files
                    .iter()
                    .any(|x| x.trash_filename == new_file_name)
                {
                    // If we get here, a file with the current name already exists in one of the trashes,
                    // so we append the current iteration number to it and check again
                    // we try to preserve the extension in case a user wants to manually recover a file
                    // (so it still has the proper extension)

                    // somefile.txt
                    let old_name = PathBuf::from(&orig_filename);

                    // somefile
                    let mut stem = old_name
                        .file_stem()
                        .unwrap_or(&orig_filename)
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

                    new_file_name = stem;

                    continue;
                } else {
                    // we have a unique filename
                    break;
                }
            }
        }

        // At this point we have a unique name, so we create the corresponding trashinfo name
        let mut trash_filename_trashinfo = new_file_name.clone();
        trash_filename_trashinfo.push(OsString::from(".trashinfo"));

        if input_file_meta.dev() == self.home_trash.device {
            // input is on the same device as the home trash, so we use that.
            let trashinfo = Trashinfo {
                trash: &self.home_trash,
                trash_filename: new_file_name,
                trash_filename_trashinfo,
                deleted_at,
                original_filepath,
            };

            self.home_trash
                .write_trashinfo(&trashinfo)
                .context("Failed to write to home trash")?;
        } else {
            let existing_trash = self
                .trashes
                .iter()
                .find(|x| x.device == input_file_meta.dev());

            if let Some(existing_trash) = existing_trash {
                // We already have a trash on the device, so we use it
                let trashinfo = Trashinfo {
                    trash: existing_trash,
                    trash_filename: new_file_name,
                    trash_filename_trashinfo,
                    deleted_at,
                    original_filepath,
                };

                existing_trash
                    .write_trashinfo(&trashinfo)
                    .context("Failed to write to trash")?;
            } else {
                let device_root =
                    find_fs_root(input_file).context("Failed to find mount point")?;

                let fs_root_meta = fs::metadata(&device_root).context("Failed to stat mount")?;
                let uid = unsafe { libc::getuid() };
                let trash_name = format!(".Trash-{}", uid);
                let trash = Trash::new_with_ensure(
                    device_root.join(trash_name),
                    device_root.clone(),
                    fs_root_meta.dev(),
                    false,
                    false,
                )
                .context(format!(
                    "Failed to create trash dir on mount: {}",
                    &device_root.display()
                ))?;

                let trashinfo = Trashinfo {
                    trash: &trash,
                    trash_filename: new_file_name,
                    trash_filename_trashinfo,
                    deleted_at,
                    original_filepath,
                };

                trash
                    .write_trashinfo(&trashinfo)
                    .context("Failed writing to trash")?;
            }
        }

        Ok(())
    }

    /// Empty the trash based on the `.trashinfo` files, meaning that files for which no
    /// `.trashinfo` file exists will be ignored
    pub fn empty(&self, before: chrono::NaiveDateTime, dry_run: bool) -> anyhow::Result<()> {
        for info in self.list().context("Failed to list trash files")? {
            if info.deleted_at < before {
                let files_file = info.trash.files_dir().join(info.trash_filename);
                let info_file = info.trash.info_dir().join(info.trash_filename_trashinfo);

                if dry_run {
                    println!("Would delete {}", info.original_filepath.display());
                    continue;
                }

                println!("Removing {}", files_file.display());
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

    /// Permanently removes a file from the trash, returning the original path of the removed file
    pub fn remove(
        &self,
        filter_predicate: impl for<'a> Fn(&Trashinfo<'a>) -> bool,
        matched_callback: impl for<'a> Fn(&'a [Trashinfo<'a>]) -> &'a Trashinfo,
    ) -> anyhow::Result<PathBuf> {
        let trashed_files = self.list().context("Failed to list trashed files")?;
        let matching = trashed_files
            .into_iter()
            .filter(filter_predicate)
            .collect::<Vec<_>>();

        let del = match matching.len() {
            0 => anyhow::bail!("No files match"),
            1 => &matching[0],
            // we only call the matched callback if more than one file matched
            _ => matched_callback(&matching),
        };

        let info_path = del.trash.info_dir().join(&del.trash_filename_trashinfo);
        let files_path = del.trash.files_dir().join(&del.trash_filename);

        if files_path.is_file() {
            fs::remove_file(&files_path).context("Failed to remove file")?;
        } else {
            fs::remove_dir_all(&files_path).context("Failed to remove directory")?;
        }

        fs::remove_file(info_path).context("Failed to remove trashinfo file")?;

        Ok(del.original_filepath.clone())
    }

    /// Restores a file to it's original location, returning the original path of the restored file
    pub fn restore(
        &self,
        filter_predicate: impl for<'a> Fn(&Trashinfo<'a>) -> bool,
        matched_callback: impl for<'a> Fn(&'a [Trashinfo<'a>]) -> &'a Trashinfo,
        exists_callback: impl for<'a> Fn(&Trashinfo<'a>) -> bool,
    ) -> anyhow::Result<PathBuf> {
        let trashed_files = self.list().context("Failed to list trashed files")?;
        let matching = trashed_files
            .into_iter()
            .filter(filter_predicate)
            .collect::<Vec<_>>();

        let restore = match matching.len() {
            0 => anyhow::bail!("No files match"),
            1 => {
                let del = &matching[0];
                if del.original_filepath.exists() && !exists_callback(del) {
                    anyhow::bail!("Aborted by user");
                }
                &matching[0]
            }
            // we only call the matched callback if more than one file matched
            _ => {
                let del = matched_callback(&matching);
                if del.original_filepath.exists() && !exists_callback(del) {
                    anyhow::bail!("Aborted by user");
                }
                del
            }
        };

        let files_path = restore.trash.files_dir().join(&restore.trash_filename);
        let info_path = restore
            .trash
            .info_dir()
            .join(&restore.trash_filename_trashinfo);

        fs::rename(&files_path, &restore.original_filepath)
            .context(f!("Failed to restore {}", files_path.display()))?;

        // We don't move the file back if this fails, as that might cause some unexpected troubles.
        fs::remove_file(&info_path).context(f!(
            "Failed to remove trashinfo file: {}",
            info_path.display()
        ))?;

        Ok(restore.original_filepath.clone())
    }
}
