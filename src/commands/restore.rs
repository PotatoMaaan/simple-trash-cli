use std::{os::unix::ffi::OsStrExt, path::PathBuf};

use anyhow::Context;

use crate::{
    commands::{ask, id_from_bytes},
    table::table,
};

pub fn restore(args: crate::cli::RestoreArgs, trash: crate::UnifiedTrash) -> anyhow::Result<()> {
    trash
        .restore(
            |trash| {
                let hash = id_from_bytes(trash.original_filepath.as_os_str().as_bytes());

                hash == args.id_or_path
                    || PathBuf::from(&args.id_or_path) == trash.original_filepath
            },
            |matched| {
                println!("Multiple files match:\n");

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
                let res = ask(&format!("Choose one [{:?}]: ", 0..matched.len()));

                todo!()
            },
        )
        .context("Failed to restore form trash")?;

    println!("Restored {}", args.id_or_path);

    Ok(())
}
