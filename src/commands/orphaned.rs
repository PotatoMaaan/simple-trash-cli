use anyhow::Context;

pub fn orphaned(
    _args: crate::cli::RemoveOrphanedArgs,
    trash: crate::UnifiedTrash,
) -> anyhow::Result<()> {
    trash
        .remove_orphaned()
        .context("Failed to remove orphaned trashinfo files")?;

    println!("Removed orphaned trashinfo files");

    Ok(())
}
