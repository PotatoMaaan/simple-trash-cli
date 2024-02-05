use anyhow::Context;
use clap::Parser;
use std::env;
use std::path::PathBuf;
use trashing::UnifiedTrash;

mod cli;
mod commands;
mod microlog;
mod table;
mod trashing;

#[cfg(test)]
mod test;

/// Based on `The FreeDesktop.org Trash specification`:
/// https://specifications.freedesktop.org/trash-spec/trashspec-latest.html at 2024-01-22
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    microlog::init(log::LevelFilter::Info);

    let bin_name = env::args()
        .next()
        .expect("How did you call a program without a path?");
    let bin_name = PathBuf::from(bin_name);
    let bin_name = bin_name
        .file_name()
        .expect("How did you call a program without a filename?")
        .to_string_lossy()
        .to_string();

    let trash = UnifiedTrash::new().context("Failed to establish a list of trash locations")?;

    match bin_name.as_str() {
        "trash" => {
            let args = cli::PutArgs::parse();
            commands::put::put(args, trash)?;
        }
        "trash-list" => {
            let args = cli::ListArgs::parse();
            commands::list::list(args, trash)?;
        }
        "trash-empty" => {
            let args = cli::EmptyArgs::parse();
            commands::empty::empty(args, trash)?
        }
        _ => {
            let root_args = cli::RootArgs::parse();
            match root_args.subcommand {
                cli::SubCmd::Put(args) => commands::put::put(args, trash)?,
                cli::SubCmd::List(args) => commands::list::list(args, trash)?,
                cli::SubCmd::Empty(args) => commands::empty::empty(args, trash)?,
                cli::SubCmd::RemoveOrphaned(args) => commands::orphaned::orphaned(args, trash)?,
            }
        }
    };

    Ok(())
}
