use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser, Clone)]
/// {n} A simple tool to interact with the XDG trashcan on linux{n}{n}
/// https://github.com/PotatoMaaan/simple-trash-cli
pub struct Args {
    #[command(subcommand)]
    pub subcommand: Commands,

    /// Ignored for rm compadibility
    #[arg(short, long)]
    pub directory: bool,

    /// Ignored for rm compadibility
    #[arg(short, long)]
    pub recursive: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Commands {
    /// Put one or more files into the trash
    Put { files: Vec<PathBuf> },

    /// Restore a file from the trash
    Restore {
        /// The original path of the file
        orig_path: PathBuf,

        /// Don't ask about replacing existing files
        #[arg(short, long)]
        force: bool,
    },

    /// Clears the trash (permanent)
    Clear,

    /// List all files in the trash
    List {
        /// Display a simple version of the output
        #[arg(short, long)]
        simple: bool,
    },

    /// Removes a single file from the trash (permanently)
    Remove { file: PathBuf },
}
