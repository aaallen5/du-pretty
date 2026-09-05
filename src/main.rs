mod parser;
mod printer;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    let input = match args.first() {
        Some(path) => match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(err) => {
                eprintln!("du-pretty: cannot read {}: {}", path, err);
                return ExitCode::from(1);
            }
        },
        None => {
            let mut buf = String::new();
            if let Err(err) = io::stdin().read_to_string(&mut buf) {
                eprintln!("du-pretty: cannot read stdin: {}", err);
                return ExitCode::from(1);
            }
            buf
        }
    };

    let entries = match parser::parse(&input) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("du-pretty: {}", err);
            return ExitCode::from(1);
        }
    };

    print!("{}", printer::print_tree(&entries));
    ExitCode::SUCCESS
}
