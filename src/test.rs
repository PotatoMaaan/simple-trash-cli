use std::{path::Path, str::FromStr};

// #[test]
// fn test_trashinfo_parse1() {
//     let ti = parse_trashinfo(Path::new("tests/testfile1.txt.trashinfo")).unwrap();

//     assert_eq!(
//         ti,
//         Trashinfo {
//             trash_filename: "testfile1.txt".into(),
//             deleted_at: chrono::NaiveDateTime::from_str("2004-08-31T22:32:08").unwrap(),
//             original_filepath: "foo/bar/meow.bow-wow".into()
//         }
//     );
// }

// #[test]
// fn test_trashinfo_parse2() {
//     let ti = parse_trashinfo(Path::new("tests/testfile2.txt.trashinfo")).unwrap();

//     assert_eq!(
//         ti,
//         Trashinfo {
//             trash_filename: "testfile2.txt".into(),
//             deleted_at: chrono::NaiveDateTime::from_str("2024-01-22T14:03:15").unwrap(),
//             original_filepath: "/home/user/Documents/files/more_files/test.rs".into()
//         }
//     );
// }

// #[test]
// fn test_trashinfo_parse3() {
//     let ti = parse_trashinfo(Path::new("tests/test file 3.trashinfo")).unwrap();
//     assert_eq!(
//         ti,
//         Trashinfo {
//             trash_filename: "test file 3".into(),
//             deleted_at: chrono::NaiveDateTime::from_str("1990-01-12T17:17:40").unwrap(),
//             original_filepath: "/home/user/testdir/file containing spaces v2.10".into()
//         }
//     );
// }

use std::{path::PathBuf, process::Command};

use crate::trashing::UnifiedTrash;

#[test]
fn me_when() {
    let trash = UnifiedTrash::new().unwrap();

    let gio_output = Command::new("gio")
        .arg("trash")
        .arg("--list")
        .output()
        .unwrap()
        .stdout;
    let gio_output = String::from_utf8(gio_output).unwrap();
    let mut gio_output = gio_output
        .lines()
        .map(|x| x.split("\t").skip(1).next().unwrap())
        .map(PathBuf::from)
        .collect::<Vec<_>>();

    let mut our_output = trash
        .list()
        .unwrap()
        .into_iter()
        .map(|x| x.original_filepath)
        .collect::<Vec<_>>();

    our_output.sort();
    gio_output.sort();

    assert_eq!(our_output, gio_output);
}
