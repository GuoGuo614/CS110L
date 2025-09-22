use std::env;
use std::process;
use std::fs::File; // For read_file_lines()
use std::io::{self, BufRead}; // For read_file_lines()

/// Reads the file at the supplied path, and returns a vector of strings.
fn read_file_lines(filename: &String) -> Result<Vec<String>, io::Error> {
    let mut str_vec = Vec::new();
    let file = File::open(filename)?;
    for line in io::BufReader::new(file).lines() {
        let line_str = line?;
        str_vec.push(line_str);
    }
    Ok(str_vec)
}

fn count_for_lines(file_vec: &Vec<String>) -> usize {
    file_vec.len()
}

fn count_for_words(lines: &Vec<String>) -> usize {
    lines.iter()
         .map(|line| line.split_whitespace().count())
         .sum()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Too few arguments.");
        process::exit(1);
    }
    let filename = &args[1];
    // Your code here :)
    let file_vec: Vec<String> = read_file_lines(filename).expect("Invalid filename of args1");
    println!("Count for lines: {}", count_for_lines(&file_vec));
    println!("Count for words: {}", count_for_words(&file_vec));
}
