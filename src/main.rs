use anyhow::Context;
use clap::Parser;
use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, stdin, stdout, BufRead, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use trashinfo::Trashinfo;

mod cli;
mod trashinfo;

#[cfg(test)]
mod test;

/// Based on `The FreeDesktop.org Trash specification`: https://specifications.freedesktop.org/trash-spec/trashspec-latest.html at 2024-01-22
fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let home_dir = PathBuf::from(env::var("HOME").expect("No home dir set!"));
    let xdg_data_dir = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home_dir.join(".local").join("share"));

    //let xdg_data_dir = PathBuf::from("xdg_data");

    let files_dir = xdg_data_dir.join("Trash").join("files");
    let info_dir = xdg_data_dir.join("Trash").join("info");

    let files_device = fs::metadata(&files_dir)
        .context("Failed to get metadata for file dir")?
        .dev();

    ensure_trash_dirs(&files_dir, &info_dir).context("Failed to create trash dirs")?;

    match args.subcommand {
        cli::Commands::Put { files } => {
            for file in files {
                if fs::metadata(&file).context("Failed to get metadata")?.dev() != files_device {
                    eprintln!("{} is on diffent filesystem, skipping", file.display());
                    continue;
                }

                let current_filename: PathBuf =
                    file.file_name().context("File has no filename")?.into();

                let mut new_info = Trashinfo {
                    trash_filename: current_filename.clone(),
                    deleted_at: chrono::Local::now().naive_local(),
                    original_filepath: PathBuf::from(
                        file.clone()
                            .canonicalize()
                            .context("Failed to resolve input path")?,
                    ),
                };

                // start at 1 because having filename0.trashinfo looks weird
                for iters in 1.. {
                    match OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .open(info_dir.join(&new_info.trash_filename.with_extension("trashinfo")))
                    {
                        Ok(mut handle) => {
                            let info_file = new_info.to_trashinfo_file();
                            handle
                                .write_all(info_file.as_bytes())
                                .context("Failed writing out info file")?;
                            break;
                        }
                        Err(err) => {
                            if err.kind() != io::ErrorKind::AlreadyExists {
                                let x = anyhow::anyhow!("Failed writing info file {}", err);
                                anyhow::bail!(x);
                            }

                            // the file already exists, so we append the current iteration number and try again
                            let mut name_changed =
                                new_info.trash_filename.clone().as_os_str().to_owned();
                            name_changed.push(OsString::from(iters.to_string()));

                            new_info.trash_filename.set_file_name(name_changed);
                            continue;
                        }
                    }
                }

                match fs::rename(
                    new_info.original_filepath,
                    files_dir.join(&new_info.trash_filename),
                ) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("ERROR: Failed moving file, reverting info file...");
                        eprintln!("ERROR: {}", e);
                        fs::remove_file(
                            info_dir
                                .join(&new_info.trash_filename)
                                .with_extension("trashinfo"),
                        )
                        .context("Failed to remove existing info file")?;
                    }
                }

                println!("trashed {}", file.display());
            }
        }
        cli::Commands::Restore { orig_path, force } => {
            if fs::metadata(&orig_path)
                .context("Failed to get metadata")?
                .dev()
                != files_device
            {
                anyhow::bail!("{} is on diffent filesystem, aborting", orig_path.display());
            }

            let selected_file =
                select_file(&info_dir, &orig_path).context("Failed to select a unique file")?;

            assert_eq!(orig_path, selected_file.original_filepath,);

            if !force && selected_file.original_filepath.exists() {
                let line = ask(&format!(
                    "A file already exists at {}\nDo you want to overwrite it? [y/N] ",
                    selected_file.original_filepath.display()
                ))
                .context("No input given")?;
                if line.to_lowercase() != "y" {
                    anyhow::bail!("Aborted by user");
                }
                println!();
            }

            fs::rename(files_dir.join(&selected_file.trash_filename), orig_path)
                .context("Failed to move file to original location")?;

            fs::remove_file(
                info_dir.join(&selected_file.trash_filename.with_extension("trashinfo")),
            )
            .context("Failed to remove info file")?;

            println!("restored\t{}", selected_file.original_filepath.display());
        }
        cli::Commands::List { simple } => {
            let parsed_info = parse_info_dir(&info_dir).context("Failed to parse info dir")?;

            match simple {
                true => {
                    for file in &parsed_info {
                        println!("{}\t{}", file.deleted_at, file.original_filepath.display());
                    }
                }
                false => {
                    println!("deleted at          | path");
                    println!("--------------------+--------------------");
                    for file in &parsed_info {
                        println!("{} | {}", file.deleted_at, file.original_filepath.display());
                    }

                    println!("\nTotal: {} files", parsed_info.len());
                }
            }
        }
        cli::Commands::Clear => {
            let line =
                ask("Are you sure you want to permanently delete all files from the trash? [y/N] ")
                    .context("No input given")?;
            if line.to_lowercase() != "y" {
                anyhow::bail!("Aborted by user");
            }

            fs::remove_dir_all(&info_dir).context("Failed to remove info dir")?;
            fs::remove_dir_all(&files_dir).context("Failed to remove file dir")?;

            ensure_trash_dirs(&files_dir, &info_dir).context("Failed to create trash dirs")?;

            println!("Cleared trash");
        }
        cli::Commands::Remove { file } => {
            let selected_file =
                select_file(&info_dir, &file).context("Failed to select a unique file")?;

            let line = ask(&format!(
                "This would permanetly remove {}?\nContinue? [y/N] ",
                &file.display()
            ))
            .context("No input provided")?;

            if line.to_lowercase() != "y" {
                anyhow::bail!("Aborted by user");
            }

            fs::remove_file(
                &info_dir.join(selected_file.trash_filename.with_extension("trashinfo")),
            )
            .context("Failed to remove info file")?;

            fs::remove_file(&files_dir.join(selected_file.trash_filename))
                .context("Failed to remove file from files dir")?;

            println!("removed\t{}", file.display());
        }
    }

    Ok(())
}

fn parse_info_dir(info_dir: &Path) -> Result<Vec<Trashinfo>, anyhow::Error> {
    let infos = fs::read_dir(&info_dir).context("Failed reading info files dir")?;
    let mut parsed_info = vec![];
    for info in infos {
        let info = info.context("Failed to get dir entry")?;
        let info = trashinfo::parse_trashinfo(&info.path()).context(format!(
            "Failed to parse info file at {}",
            info.path().display()
        ))?;
        parsed_info.push(info);
    }
    Ok(parsed_info)
}

fn ask(txt: &str) -> Option<String> {
    print!("{}", txt);
    stdout().flush().expect("Failed to flush stdout");
    stdin().lock().lines().next()?.ok()
}

fn ensure_trash_dirs(files_dir: &Path, info_dir: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(&files_dir).context("Failed to create trash files dir")?;
    fs::create_dir_all(&info_dir).context("Failed to create trash info dir")?;
    Ok(())
}

fn select_file(info_dir: &Path, select_path: &Path) -> anyhow::Result<Trashinfo> {
    let parsed_info = parse_info_dir(info_dir).context("Failed to parse info dir")?;
    let matched_files = parsed_info
        .into_iter()
        .filter(|x| x.original_filepath == select_path)
        .collect::<Vec<_>>();

    if matched_files.is_empty() {
        anyhow::bail!("No trashed files matched the given path");
    }

    Ok(if matched_files.len() > 1 {
        eprintln!("Multiple versions found, choose one:");
        for (idx, file) in matched_files.iter().enumerate() {
            println!(
                "{idx}: {}\t{}",
                file.deleted_at,
                file.original_filepath.display()
            );
        }
        let line = ask(&format!("{:?}: ", 0..matched_files.len() - 1)).context("No input given")?;
        let line: usize = line.parse().context("Not a valid number")?;
        println!();

        matched_files.get(line).context("Invalid file index")?
    } else {
        &matched_files[0]
    }
    .clone())
}
