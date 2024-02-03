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
    pub fn trashinfo_file(&self) -> String {
        self.create_trashfile(&self.original_filepath)
    }

    fn create_trashfile(&self, orig_filepath: &Path) -> String {
        let encoded = urlencoding::encode_binary(orig_filepath.as_os_str().as_bytes());
        format!(
            "[Trash Info]\nPath={}\nDeletionDate={}",
            encoded,
            // The same format that nautilus and dolphin use. The spec claims rfc3339, but that doesn't work out at all...
            self.deleted_at.format("%Y-%m-%dT%H:%M:%S")
        )
    }

    pub fn trashinfo_file_relative(&self, relative_to: &Path) -> anyhow::Result<String> {
        let relative_path = self
            .original_filepath
            .strip_prefix(relative_to)
            .context("Failed to strip prefix")?;

        assert!(relative_path.is_relative());

        Ok(self.create_trashfile(relative_path))
    }
}

/// location: Path of the .trashinfo file
/// dev_root: Path where the trash dir is inside, not the path of the actual trash dir!
pub fn parse_trashinfo(location: &Path, dev_root: &Path) -> anyhow::Result<Trashinfo> {
    let file = fs::read_to_string(location)
        .context("Failed reading trashinfo file, this is probably a bug")?;

    let mut lines = file.lines();

    // Its first line must be [Trash Info].
    if lines.next().context("no first line")? != "[Trash Info]" {
        anyhow::bail!("invalid first line");
    }

    fn parse_line(line: &str) -> anyhow::Result<(&str, &str)> {
        let mut line = line.split("=");
        let key = line.next().context("No key")?;
        let val = line.next().context("No Value")?;

        Ok((key, val))
    }

    // The implementation MUST ignore any other lines in this file, except the first line (must be [Trash Info]) and these two key/value pairs.
    // If a string that starts with “Path=” or “DeletionDate=” occurs several times, the first occurence is to be used
    let lines = lines
        .map(parse_line)
        .collect::<anyhow::Result<FxHashMap<&str, &str>>>()
        .context("invalid line (s)")?;

    let path = *lines.get("Path").context("no Path entry")?;

    // Unlike Rust strings, paths on unix / linux don't have to be utf-8,
    // so we decode to binary and construct a Path from the bytes, which can be any sequence of bytes.
    let path = urlencoding::decode_binary(path.as_bytes()).to_vec();
    let path = OsStr::from_bytes(&path);
    let path = Path::new(path);

    // if the found path is relative, it's based on the dev_root
    let path = if path.is_relative() {
        dev_root.join(path)
    } else {
        path.to_path_buf()
    };

    let deleted_at = *lines.get("DeletionDate").context("No DeletionDate entry")?;

    /// This covers most real-world cases
    fn parser1(input: &str) -> Result<NaiveDateTime, chrono::ParseError> {
        chrono::NaiveDateTime::from_str(&input)
    }

    /// According to the spec, the datetime should be rfc3339, but i've not found a single real example that actually works here
    /// Even the provided sample time in the spec does not parse with this.
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

    // probably horribly over complicated, but i really wanted to get the errors from each parser
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

    let parsed_datetime = oks
        .first()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "all parsers failed: {:?}",
                errs.iter().map(|x| format!("{x}")).collect::<Vec<_>>()
            )
        })
        .context("invalid datetime")?
        .to_owned();

    Ok(Trashinfo {
        trash_filename: location.file_stem().context("no file name")?.into(),
        deleted_at: parsed_datetime,
        original_filepath: path.to_path_buf(),
    })
}
