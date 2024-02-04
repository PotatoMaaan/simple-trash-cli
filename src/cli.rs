use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
/// A program to interact with the XDG Trash.
pub struct RootArgs {
    #[command(subcommand)]
    pub subcommand: SubCmd,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SubCmd {
    Put(PutArgs),
    List(ListArgs),
}

#[derive(Debug, Clone, Parser)]
/// Put files into the trash
pub struct PutArgs {
    pub files: Vec<PathBuf>,
}

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

#[derive(Debug, Clone, ValueEnum)]
pub enum Sorting {
    Trash,
    OriginalPath,
    DeletedAt,
}
