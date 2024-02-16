use crate::{table::table, trashing::UnifiedTrash};

pub fn list_trashes(_args: crate::cli::ListTrashesArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    let trashes = trash.list_trashes();

    let trashes_table = trashes
        .into_iter()
        .map(|x| {
            [
                x.trash_path.to_string_lossy().to_string(),
                x.dev_root.to_string_lossy().to_string(),
                x.device.to_string(),
                x.is_admin_trash.to_string(),
                x.is_home_trash.to_string(),
            ]
        })
        .collect::<Vec<_>>();

    table(
        &trashes_table,
        [
            "Path",
            "Device root",
            "Device ID",
            "Is admin created",
            "Is home trash",
        ],
    );

    Ok(())
}
