use std::path::Path;

use crate::trashinfo::*;

#[test]
fn test_trashinfo_parse1() {
    let ti = parse_trashinfo(Path::new("tests/testfile1.txt.trashinfo")).unwrap();

    assert_eq!(
        ti,
        Trashinfo {
            trash_filename: todo!(),
            deleted_at: todo!(),
            original_filepath: todo!()
        }
    );
}

#[test]
fn test_trashinfo_parse2() {
    let ti = parse_trashinfo(Path::new("tests/testfile2.txt.trashinfo")).unwrap();

    assert_eq!(
        ti,
        Trashinfo {
            trash_filename: todo!(),
            deleted_at: todo!(),
            original_filepath: todo!()
        }
    );
}

// #[test]
// fn test_trashinfo_parse3() {
//     let ti = parse_trashinfo(Path::new("tests/test file 3.trashinfo")).unwrap();

//     assert_eq!(
//         ti,
//         Trashinfo {
//             file: PathBuf::from("test file 3"),
//             deleted_at: "1990-01-12T17:17:40".to_owned(),
//             original_filepath: PathBuf::from("/home/user/testdir/file containing spaces v2.10"),
//         }
//     );
// }
