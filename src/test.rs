use crate::trashing::UnifiedTrash;
use std::{path::PathBuf, process::Command};

#[test]
fn test_trash_list() {
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

    let mut difference = vec![];
    for i in &our_output {
        if !gio_output.contains(&i) {
            difference.push(i);
        }
    }

    assert_eq!(our_output, gio_output, "DIFFERENCE: {:?}\n\n", difference);
}
