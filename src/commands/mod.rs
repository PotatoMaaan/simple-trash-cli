use colored::Colorize;
use sha2::Digest;
use sha2::Sha256;
use std::fmt::Write;
use std::io::stdin;
use std::io::stdout;
use std::io::BufRead;
use std::io::Write as _;

pub mod empty;
pub mod list;
pub mod list_trashes;
pub mod orphaned;
pub mod put;
pub mod remove;
pub mod restore;

pub fn id_from_bytes(input: &[u8]) -> String {
    let hash = Sha256::digest(input);
    let hash = hash.as_slice();
    encode_hex(hash).chars().take(10).collect()
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

pub fn ask(prompt: &str) -> String {
    print!("{}", prompt);
    stdout().flush().expect("Failed to flush stdout");
    stdin()
        .lock()
        .lines()
        .next()
        .unwrap_or(Ok("".to_owned()))
        .unwrap_or("".to_owned())
}

pub fn ask_yes_no(prompt: &str, default: bool) -> bool {
    let p = ask(&format!(
        "{} [{}] ",
        prompt,
        match default {
            true => "Y/n".green(),
            false => "y/N".bright_red(),
        }
    ));

    match (p.to_lowercase().as_str(), default) {
        ("n", true) => true,
        ("y", false) => true,
        _ => false,
    }
}
