use std::{os::unix::ffi::OsStrExt, path::PathBuf, process::exit};

use anyhow::Context;
use log::error;

use crate::{
    commands::{ask, ask_yes_no, id_from_bytes},
    table::table,
};

pub fn restore(args: crate::cli::RestoreArgs, trash: crate::UnifiedTrash) -> anyhow::Result<()> {
    let restored = trash
        .restore(
            |info| {
                let hash = id_from_bytes(info.original_filepath.as_os_str().as_bytes());

                hash == args.id_or_path || PathBuf::from(&args.id_or_path) == info.original_filepath
            },
            |matched| {
                println!("Multiple files match {}:\n", args.id_or_path);

                let mut collector = vec![];
                for (i, info) in matched.iter().enumerate() {
                    collector.push([
                        i.to_string(),
                        args.id_or_path.to_string(),
                        info.deleted_at.to_string(),
                    ]);
                }
                table(&collector, ["Index", "File", "Deleted At"]);
                println!();
                let res: usize = ask(&format!("Choose one [{:?}]: ", 0..matched.len() - 1))
                    .parse()
                    .unwrap_or_else(|e| {
                        error!("Invalid number: {}", e);
                        exit(1);
                    });

                if let Some(t) = matched.get(res) {
                    t
                } else {
                    error!("Index {} does not exist", res);
                    exit(1);
                }
            },
            |info| {
                if !ask_yes_no(
                    &format!(
                        "A file already exists at '{}', do you want to overwrite it?",
                        info.original_filepath.display()
                    ),
                    false,
                ) {
                    error!("Aborted by user");
                    exit(0);
                }
                true
            },
        )
        .context("Failed to restore form trash")?;

    println!("Restored {}", restored.display());

    Ok(())
}
