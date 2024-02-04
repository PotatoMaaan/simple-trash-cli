use anyhow::Context;

use crate::{cli, trashing::UnifiedTrash};

pub fn put(args: cli::PutArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    trash.put(&args.files).context("Failed to trash files")
}
