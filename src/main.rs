use clap::Parser;

mod cli;
mod trashing;

#[cfg(test)]
mod test;

/// Based on `The FreeDesktop.org Trash specification`:
/// https://specifications.freedesktop.org/trash-spec/trashspec-latest.html at 2024-01-22
#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use trashing::UnifiedTrash;

    let args = cli::Args::parse();

    let trash = UnifiedTrash::new().unwrap();

    match args.subcommand {
        cli::Commands::Put { files } => {
            trash.put(&files)?;
        }
        cli::Commands::Restore { orig_path, force } => {
            todo!()
        }
        cli::Commands::List { simple } => {
            for f in trash.list()? {
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
