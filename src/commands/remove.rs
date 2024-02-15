use std::process::exit;

use anyhow::Context;
use log::error;

use crate::{commands::ask, table::table, trashing::UnifiedTrash};

pub fn remove(args: crate::cli::RemoveArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    trash
        .remove(
            |trash| {
                println!("");
                true
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

                let res: usize = ask(&format!("Choose one [{:?}]: ", 0..matched.len()))
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
        )
        .context("Failed to remove file")
}
