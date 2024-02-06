use std::{
    ffi::{OsStr, OsString},
    fs,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use chrono::NaiveDateTime;
use rustc_hash::FxHashMap;

use super::Trash;

/// Information about a trashed file
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Trashinfo<'a> {
    pub trash: &'a Trash,

    /// Filename to be found in the `files` directory.
    /// Not explicity mentioned by the spec.
    /// Does not include `trashinfo` extension
    pub trash_filename: OsString,

    /// the same as `trash_filename` but with `.trashinfo` *appended* to the end.
    pub trash_filename_trashinfo: OsString,

    /// `DeletionDate` in the spec (local time)
    pub deleted_at: NaiveDateTime,

    /// `Path` in the spec
    pub original_filepath: PathBuf,
}

impl<'a> Trashinfo<'a> {
    /// Creates a trashinfo file from the current state
    ///
    /// Uses absolute paths, see `trashinfo_file_relative` for relative paths
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

    /// Creates a trashinfo file from the current state using relative paths
    ///
    /// Accoding to the spec, implementations should use relative paths any trash
    /// but the home trash. This makes it possible to still use the trash even if
    /// the drive is mounted to a different path
    pub fn trashinfo_file_relative(&self, relative_to: &Path) -> anyhow::Result<String> {
        let relative_path = self
            .original_filepath
            .strip_prefix(relative_to)
            .context("Failed to strip prefix")?;

        assert!(relative_path.is_relative());

        Ok(self.create_trashfile(relative_path))
    }

    /// Renames `self` to the `new_name`
    ///
    /// ## Important
    /// This method *always* adds the `.trashinfo` extension
    pub fn rename(&mut self, new_name: OsString) {
        dbg!(&self);
        self.trash_filename = new_name.clone();
        let mut new_name_trashinfo = new_name;
        new_name_trashinfo.push(OsString::from(".trashinfo"));
        self.trash_filename_trashinfo = new_name_trashinfo;
        dbg!(&self);
    }
}

/// Attempts to parse a `.trashinfo` fole at the `location`.
///
/// `dev_root` Path where the trash resides, not the path of the actual trash dir!
/// The original location of the parsed file is based off this (if it is relative),
/// so be careful.
/// ## Example:
/// location: `/mnt/drive1/.Trash-1000/info/somefile.trashinfo`
///
/// dev_root: `/mnt/drive1`
pub fn parse_trashinfo<'a>(location: &Path, trash: &'a Trash) -> anyhow::Result<Trashinfo<'a>> {
    let file = fs::read_to_string(location).context("Failed reading trashinfo file")?;

    let mut lines = file.lines();

    // the first line must be [Trash Info].
    if lines.next().context("no first line")? != "[Trash Info]" {
        anyhow::bail!("invalid first line");
    }

    fn parse_line(line: &str) -> anyhow::Result<(&str, &str)> {
        let mut line = line.split("=");
        let key = line.next().context("No key")?;
        let val = line.next().context("No Value")?;

        Ok((key, val))
    }

    // the implementation MUST ignore any other lines in this file, except the first line (must be [Trash Info]) and these two key/value pairs.
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

    // when partition_map() in std :(
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
        trash_filename_trashinfo: location.file_name().context("No file name")?.to_os_string(),
        deleted_at: parsed_datetime,
        original_filepath: path.to_path_buf(),
    })
}

#[test]
fn test_trashinfo_parse1() {
    let ti = parse_trashinfo(Path::new("tests/testfile1.txt.trashinfo"), &Path::new("")).unwrap();

    assert_eq!(
        ti,
        Trashinfo {
            trash_filename: "testfile1.txt".into(),
            trash_filename_trashinfo: "testfile1.txt.trashinfo".into(),
            deleted_at: chrono::NaiveDateTime::from_str("2004-08-31T22:32:08").unwrap(),
            original_filepath: "foo/bar/meow.bow-wow".into(),
        }
    );
}

#[test]
fn test_trashinfo_parse2() {
    let ti = parse_trashinfo(Path::new("tests/testfile2.txt.trashinfo"), &Path::new("")).unwrap();

    assert_eq!(
        ti,
        Trashinfo {
            trash_filename: "testfile2.txt".into(),
            trash_filename_trashinfo: "testfile2.txt.trashinfo".into(),
            deleted_at: chrono::NaiveDateTime::from_str("2024-01-22T14:03:15").unwrap(),
            original_filepath: "/home/user/Documents/files/more_files/test.rs".into()
        }
    );
}

#[test]
fn test_trashinfo_parse3() {
    let ti = parse_trashinfo(Path::new("tests/test file 3.trashinfo"), &Path::new("")).unwrap();

    assert_eq!(
        ti,
        Trashinfo {
            trash_filename_trashinfo: "test file 3.trashinfo".into(),
            trash_filename: "test file 3".into(),
            deleted_at: chrono::NaiveDateTime::from_str("1990-01-12T17:17:40").unwrap(),
            original_filepath: "/home/user/testdir/file containing spaces v2.10".into()
        }
    );
}
