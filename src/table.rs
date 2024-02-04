// This whole thing is kinda messy, but it works :)

use colored::{ColoredString, Colorize};

/// Print a pretty table
pub fn table<const COLS: usize>(data: &[[String; COLS]], headers: [&str; COLS]) {
    #[allow(non_snake_case)]
    let VERTICAL: ColoredString = " | ".bright_black();
    #[allow(non_snake_case)]
    let HORIZONTAL: ColoredString = "-".bright_black();
    #[allow(non_snake_case)]
    let INTERSECTION: ColoredString = "-+-".bright_black();

    let mut longest = [0; COLS];
    for row in data {
        for (i, row) in row.iter().enumerate() {
            longest[i] = row.chars().count().max(longest[i]);
        }
    }

    for (i, row) in headers.iter().enumerate() {
        longest[i] = row.chars().count().max(longest[i]);
    }

    for (col_idx, header) in headers.iter().enumerate() {
        print!("{}", pad(header, longest[col_idx], " ").white());
        if col_idx + 1 != COLS {
            print!("{}", VERTICAL)
        }
    }
    println!();

    for col_idx in 0..COLS {
        print!("{}", pad_col("", longest[col_idx], &HORIZONTAL));
        if col_idx + 1 != COLS {
            print!("{}", INTERSECTION)
        }
    }
    println!();

    for row in data {
        for (col_idx, item) in row.iter().enumerate() {
            print!("{}", item);
            if col_idx + 1 != COLS {
                print!(
                    "{}{}",
                    pad("", longest[col_idx] - item.chars().count(), " "),
                    VERTICAL
                )
            }
        }
        println!();
    }
}

fn pad(input: &str, mut len: usize, c: &str) -> String {
    let in_chars = input.chars().count();
    if in_chars > len {
        len = in_chars;
    }
    input.to_string() + &c.repeat(len - in_chars)
}

fn pad_col(input: &str, mut len: usize, c: &ColoredString) -> ColoredString {
    let in_chars = input.chars().count();
    if in_chars > len {
        len = in_chars;
    }
    let o = input.to_string() + &c.repeat(len - in_chars);
    o.color(c.fgcolor().unwrap_or(colored::Color::BrightWhite))
}
