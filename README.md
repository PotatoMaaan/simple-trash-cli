# Simple-Trash-Cli

The goal of this project was to build a simpler and faster version of [trash-cli](https://github.com/andreafrancia/trash-cli). The project has as little depencies as possible.

I tried to be as compliant with [the XDG spec](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html) as reasonably possible, with the exception of top-level trash directores. (might be supported in the future)

> [!IMPORTANT]
> I made this mainly for personal use. While you can of course use it yourself, be advised that I cannot guarantee the integrity of your files. Use at your own risk.

### Currently missing features (might come later)

- Invoking subcommands through the binary name directly (eg. calling `trash-restore`)
- Top-level trash directores
- Restore multiple files
- Remove / Restore based on pattern
- Listing on various repos (eg. AUR)

# Installation / Building

### Pre-compiled

You can grab a pre-compiled binary from the releases or build one yourself (see below)

### Building

To build the project, you just need a rust toolchain installed. Get it here [here](https://rustup.rs/).

Now you can run:

```sh
cargo build --release #builds an optimized binary
```

The binary will be in `target/release`

### Testing

In order to run the tests, just run

```
cargo test
```

# Usage

Here is a list of available subcommands

| Command | Usage                                              |
| ------- | -------------------------------------------------- |
| put     | Put one or more files into the trash               |
| restore | Restore a file from the trash                      |
| clear   | Clears the trash (permanent)                       |
| list    | List all files in the trash                        |
| remove  | Removes a single file from the trash (permanently) |

# Contributing

If you find a bug feel free to open an issue.
