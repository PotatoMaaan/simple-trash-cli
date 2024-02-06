use anyhow::Context;
use format as f;
use log::error;

use crate::{cli, trashing::UnifiedTrash};

pub fn put(args: cli::PutArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    for file in args.files {
        if args.force {
            if let Err(err) = trash.put(&file) {
                error!("Failed to trash {}: {}", file.display(), err);
            }
        } else {
            trash
                .put(&file)
                .context(f!("Failed to trash {}", file.display()))?;
        }
    }

    Ok(())
}
