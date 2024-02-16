# Simple-Trash-Cli

A simple tool for interacting with the XDG trash, similar to [trash-cli](https://github.com/andreafrancia/trash-cli). The projects aims to be straightforward and well documented.

You only need a single, small binary, making this ideal for minimal server setups etc.

I tried to be as compliant with [the XDG spec](https://specifications.freedesktop.org/trash-spec/trashspec-latest.html) as reasonably possible, diverging in some small places where other implementations (notably glib) also do so.

> [!IMPORTANT]
> I made this mainly for personal use. While I tried my best to ensure integrity, I cannot guarantee this. Use at your own risk.

# Installation / Building

### Pre-compiled

You can grab a pre-compiled binary from the releases or build one yourself (see below)

### Building

To build the project, you just need a rust toolchain installed. Get it here [here](https://rustup.rs/).

Now you can run:

```sh
cargo build --release #builds an optimized binary

cargo build --release --target x86_64-unknown-linux-musl #(optional) statically links against musl to avoid possible libc version mismatches.
```

You'll find the binary in `target/release`

### Testing

In order to run the tests, just run

```
cargo test
```

# Usage

Here are some example commands and their outputs:

```sh
$ trash-cli list

ID         | Deleted at          | Original location
-----------+---------------------+--------------------------------------------------------------------------
e93c362f7a | 2024-02-15 16:23:38 | /home/user/Documents/somefile.txt
fac3d34e15 | 2024-02-12 12:26:05 | /home/user/Downloads/file.zip
6203242363 | 2024-01-17 18:42:20 | /home/user/Downloads/other file.mp4
67b927f4b0 | 2024-01-12 11:34:52 | /home/user/Downloads/garbled_filename.mp4
```

```sh
$ trash-cli restore  67b927f4b0 #You can restore a file based on this id if the name is garbled or too long

Restored /home/user/Downloads/garbled_filename.mp4
```

```sh
$ trash random_file.jpg #calls the binary with the name of the subcommand directly

Trashed /home/user/Downloads/random_file.jpg
```

```sh
$ trash-cli list-trashes

Path                                        | Relative root                    | Device ID
--------------------------------------------+----------------------------------+----------
/home/user/.local/share/Trash               | /home/user/.local/share          | 66306
/mnt/extdrive/.Trash/1000                   | /mnt/extdrive                    | 2049
/home/user/mount/some-smb-share/.Trash-1000 | /home/user/mount/some-smb-share  | 59
```

Run `trash-cli --help` to see a list of all available commands.

## Reporting bugs

If you find a bug feel free to open an issue.
