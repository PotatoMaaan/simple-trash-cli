use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
/// A program to interact with the XDG Trash.{n}{n}
/// Note:{n}
/// Some Subcommands show an ID column, this ID can be used to{n}
/// uniquely identify files even if the filename contains otherwise unprintable bytes.{n}{n}
/// This program supports being called through the following names to directly call the subcommand:{n}{n}
/// trash       -> trash put{n}
/// trash-put   -> trash put{n}
/// trash-list  -> trash list{n}
/// trash-empty -> trash empty{n}
pub struct RootArgs {
    #[command(subcommand)]
    pub subcommand: SubCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SubCmd {
    Put(PutArgs),
    List(ListArgs),
    Empty(EmptyArgs),
    RemoveOrphaned(RemoveOrphanedArgs),
    Restore(RestoreArgs),
    Remove(RemoveArgs),
}

#[derive(Debug, Clone, Parser)]
/// Put files into the trash
pub struct PutArgs {
    /// One or more file(s) to trash
    pub files: Vec<PathBuf>,

    /// Continue on errors (errors will still be logged to stderr)
    #[arg(short, long)]
    pub force: bool,

    /// Does nothing, exists for compadibility with rm
    #[arg(short, long)]
    pub recursive: bool,
}

/// List trashed files
#[derive(Debug, Clone, Parser)]
pub struct ListArgs {
    /// Just output columnns seperated by \t (for easy parsing) (2>/dev/null to ignore erros / warnings)
    #[arg(short, long)]
    pub simple: bool,

    /// Also display the trash location where each file resides
    #[arg(short, long)]
    pub trash_location: bool,

    /// Reverse the sorting
    #[arg(short, long)]
    pub reverse: bool,

    /// Sort by this value
    #[arg(long, value_enum, default_value_t = Sorting::OriginalPath)]
    pub sort: Sorting,
}

/// Empty the trash
#[derive(Debug, Clone, Parser)]
pub struct EmptyArgs {
    /// Only delete files that were trashed before the specified date (format example: 2024-01-24)
    #[arg(short = 'b', long)]
    pub before_date: Option<chrono::NaiveDate>,

    /// Same as before-date but including a time (format example: 2024-01-24T16:27:00)
    #[arg(short = 'B', long)]
    pub before_datetime: Option<chrono::NaiveDateTime>,

    /// Dry run. Don't delete anything, just print.
    #[arg(short, long)]
    pub dry_run: bool,
}

/// Remove orphaned trashinfo files
#[derive(Debug, Clone, Parser)]
pub struct RemoveOrphanedArgs {}

/// Restore a file from the trash
#[derive(Debug, Clone, Parser)]
pub struct RestoreArgs {
    /// The ID of a file or it's original
    pub id_or_path: String,
}

/// Permanently remove a file from the trash
#[derive(Debug, Clone, Parser)]
pub struct RemoveArgs {
    /// The ID of a file or it's original
    pub id_or_path: String,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum Sorting {
    Trash,
    OriginalPath,
    DeletedAt,
}
