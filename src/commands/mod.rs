use sha2::Digest;
use sha2::Sha256;
use std::fmt::Write;

pub mod list;
pub mod put;

pub fn hash(input: &[u8]) -> String {
    let hash = Sha256::digest(input);
    let hash = hash.as_slice();
    encode_hex(hash)
}

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}
