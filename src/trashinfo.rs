use std::{
    ffi::OsStr,
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use chrono::NaiveDateTime;
use rustc_hash::FxHashMap;

/// Information about a trashed file
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Trashinfo {
    /// Filename to be found in the `files` directory.
    /// Not explicity mentioned by the spec.
    /// Does not include `trashinfo` extension
    pub trash_filename: PathBuf,

    /// `DeletionDate` in the spec (local time)
    pub deleted_at: NaiveDateTime,

    /// `Path` in the spec
    pub original_filepath: PathBuf,
}

impl Trashinfo {
    pub fn to_trashinfo_file(&self) -> String {
        let encoded = urlencoding::encode_binary(self.original_filepath.as_os_str().as_bytes());
        format!(
            "[Trash Info]\nPath={}\nDeletionDate={}",
            encoded,
            // The same format that nautilus uses. The spec claims rfc3339, but that does not work on so many levels
            self.deleted_at.format("%Y-%m-%dT%H:%M:%S")
        )
    }
}

pub fn parse_trashinfo(location: &Path) -> anyhow::Result<Trashinfo> {
    let file = fs::read_to_string(location)
        .context("Failed reading trashinfo file, this is probably a bug")?;
    let mut lines = file.lines();

    // >Its first line must be [Trash Info].
    if lines.next().context("no first line")? != "[Trash Info]" {
        anyhow::bail!("invalid first line");
    }

    fn parse_line(line: &str) -> Option<(&str, &str)> {
        let mut line = line.split("=");
        let key = line.next().context("No key").ok()?;
        let val = line.next().context("No Value").ok()?;

        Some((key, val))
    }

    // The implementation MUST ignore any other lines in this file, except the first line (must be [Trash Info]) and these two key/value pairs.
    // If a string that starts with “Path=” or “DeletionDate=” occurs several times, the first occurence is to be used
    let lines = lines
        .map(parse_line)
        .collect::<Option<FxHashMap<&str, &str>>>()
        .context("invalid line (s)")?;

    let path = *lines.get("Path").context("no Path entry")?;

    // Unlike Rust strings, paths on unix / linux don't have to be utf-8,
    // so we decode to binary and construct a Path from the bytes, which can be any sequence of bytes.
    let path = urlencoding::decode_binary(path.as_bytes()).to_vec();
    let path = OsStr::from_bytes(&path);
    let path = Path::new(path);

    let deleted_at = *lines.get("DeletionDate").context("No DeletionDate entry")?;

    /// This covers most real-world
    fn parser1(input: &str) -> Result<NaiveDateTime, chrono::ParseError> {
        chrono::NaiveDateTime::from_str(&input)
    }

    /// According to the spec, the datetime should be rfc3339, but i've not found a single real example that actually works here
    /// Even the provided sample time does not parse with this.
    fn parser2(input: &str) -> Result<NaiveDateTime, chrono::ParseError> {
        chrono::DateTime::parse_from_rfc3339(&input).map(|x| x.naive_local())
    }

    /// This works for the example provided in the spec.
    fn parser3(input: &str) -> Result<NaiveDateTime, chrono::ParseError> {
        chrono::NaiveDateTime::parse_from_str(&input, "%Y%m%dT%H:%M:%S")
    }

    /// Let's just also throw this in because why not
    fn parser4(input: &str) -> Result<NaiveDateTime, chrono::ParseError> {
        chrono::DateTime::parse_from_rfc2822(&input).map(|x| x.naive_local())
    }

    let (oks, errs) = [parser1, parser2, parser3, parser4]
        .into_iter()
        .map(|f| f(deleted_at))
        .map(|x| match x {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e)),
        })
        .fold((vec![], vec![]), |(mut oks, mut errs), x| {
            match x {
                (None, Some(e)) => errs.push(e),
                (Some(v), None) => oks.push(v),
                _ => {}
            }
            (oks, errs)
        });

    let parsed = oks
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "all parsers failed: {:?}",
                errs.iter().map(|x| format!("{x}")).collect::<Vec<_>>()
            )
        })
        .context("invalid datetime")?
        .to_owned();

    let info = Trashinfo {
        trash_filename: location.file_stem().context("no file name")?.into(),
        deleted_at: parsed,
        original_filepath: path.to_path_buf(),
    };

    Ok(info)
}
