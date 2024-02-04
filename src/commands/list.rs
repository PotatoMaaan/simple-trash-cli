use crate::{
    cli,
    commands::hash,
    table::table,
    trashing::{Trash, Trashinfo, UnifiedTrash},
};
use std::os::unix::ffi::OsStrExt;

pub fn list(args: cli::ListArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    let mut entries = vec![];

    let mut trash_list = trash.list()?;

    let sorter: fn(&(&Trash, Trashinfo), &(&Trash, Trashinfo)) -> _ = match args.sort {
        cli::Sorting::Trash => |(a, _), (b, _)| a.trash_path.cmp(&b.trash_path),
        cli::Sorting::OriginalPath => {
            |(_, a), (_, b)| a.original_filepath.cmp(&b.original_filepath)
        }
        cli::Sorting::DeletedAt => |(_, a), (_, b)| a.deleted_at.cmp(&b.deleted_at),
    };
    trash_list.sort_by(sorter);

    if args.reverse {
        trash_list.reverse();
    }

    for (trash, entry) in trash_list {
        let hash = hash(entry.original_filepath.as_os_str().as_bytes());

        entries.push([
            hash.chars().take(10).collect::<String>(),
            entry.deleted_at.to_string(),
            trash.trash_path.display().to_string(),
            entry.original_filepath.display().to_string(),
        ]);
    }

    match (args.simple, args.trash_location) {
        (true, true) => {
            for row in entries {
                println!("{}\t{}\t{}\t{}", row[0], row[1], row[2], row[3]);
            }
        }
        (true, false) => {
            for row in entries {
                println!("{}\t{}\t{}", row[0], row[1], row[3]);
            }
        }
        (false, true) => {
            table(
                &entries,
                ["ID", "Deleted at", "Trash location", "Original location"],
            );
        }
        (false, false) => {
            let mut accum2 = vec![];
            for x in entries {
                accum2.push([x[0].clone(), x[1].clone(), x[3].clone()]);
            }

            table(&accum2, ["ID", "Deleted at", "Original location"]);
        }
    }

    Ok(())
}
