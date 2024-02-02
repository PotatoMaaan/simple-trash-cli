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
mod unified_trash;

#[cfg(test)]
mod test;

/// Based on `The FreeDesktop.org Trash specification`:
/// https://specifications.freedesktop.org/trash-spec/trashspec-latest.html at 2024-01-22
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use unified_trash::UnifiedTrash;

    let args = cli::Args::parse();

    let trash = UnifiedTrash::new().unwrap();

    match args.subcommand {
        cli::Commands::Put { files } => {
            trash.put(&files).unwrap();
        }
        cli::Commands::Restore { orig_path, force } => {
            todo!()
        }
        cli::Commands::List { simple } => {
            for f in trash.list().unwrap() {
                println!(
                    "{} -> {}",
                    f.trash_filename.display(),
                    f.original_filepath.display()
                );
            }
        }
        cli::Commands::Clear => {
            todo!()
        }
        cli::Commands::Remove { file } => {
            todo!()
        }
    }

    Ok(())
}

fn parse_info_dir(info_dir: &Path) -> Result<Vec<Trashinfo>, anyhow::Error> {
    let infos = fs::read_dir(&info_dir).context("Failed reading info files dir")?;
    let mut parsed_info = vec![];
    for info in infos {
        let info = info.context("Failed to get dir entry")?;
        let info = trashinfo::parse_trashinfo(&info.path(), todo!()).context(format!(
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
