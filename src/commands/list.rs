use crate::{
    cli,
    commands::id_from_bytes,
    table::table,
    trashing::{Trashinfo, UnifiedTrash},
};
use std::os::unix::ffi::OsStrExt;

pub fn list(args: cli::ListArgs, trash: UnifiedTrash) -> anyhow::Result<()> {
    let mut entries = vec![];

    let mut trash_list = trash.list()?;

    let sorter: for<'a> fn(&Trashinfo<'a>, &Trashinfo<'a>) -> _ = match args.sort {
        cli::Sorting::Trash => |a, b| a.trash.trash_path.cmp(&b.trash.trash_path),
        cli::Sorting::OriginalPath => |a, b| a.original_filepath.cmp(&b.original_filepath),
        cli::Sorting::DeletedAt => |a, b| a.deleted_at.cmp(&b.deleted_at),
    };
    trash_list.sort_by(sorter);

    if args.reverse {
        trash_list.reverse();
    }

    for entry in trash_list {
        let id = id_from_bytes(entry.original_filepath.as_os_str().as_bytes());

        entries.push([
            id,
            entry.deleted_at.to_string(),
            entry.trash.trash_path.display().to_string(),
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
            println!();
            table(
                &entries,
                ["ID", "Deleted at", "Trash location", "Original location"],
            );
            println!();
        }
        (false, false) => {
            println!();
            let mut accum2 = vec![];
            for x in entries {
                accum2.push([x[0].clone(), x[1].clone(), x[3].clone()]);
            }

            table(&accum2, ["ID", "Deleted at", "Original location"]);
            println!();
        }
    }

    Ok(())
}
