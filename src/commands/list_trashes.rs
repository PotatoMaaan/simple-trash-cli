use crate::{table::table, trashing::UnifiedTrash};

pub fn list_trashes(args: crate::cli::ListTrashesArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    let trashes = trash.list_trashes();

    if args.simple {
        for trash in trashes {
            println!(
                "{}\t{}\t{}",
                trash.trash_path.display(),
                trash.dev_root.display(),
                trash.device
            );
        }
    } else {
        let trashes_table = trashes
            .iter()
            .map(|x| {
                [
                    x.trash_path.to_string_lossy().to_string(),
                    x.dev_root.to_string_lossy().to_string(),
                    x.device.to_string(),
                ]
            })
            .collect::<Vec<_>>();

        table(&trashes_table, ["Path", "Relative root", "Device ID"]);
    }

    Ok(())
}
