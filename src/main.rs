mod parser;
mod printer;

use printer::{PrintOptions, SortMode};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    let (opts, path) = match parse_args(&args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("du-pretty: {}", err);
            return ExitCode::from(1);
        }
    };

    let input = match &path {
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

    print!("{}", printer::print_tree(&entries, &opts));
    ExitCode::SUCCESS
}

// Only one positional argument (the report path) is accepted; everything
// else must be a recognized `--flag=value` so typos fail loudly instead of
// being read as a second file path.
fn parse_args(args: &[String]) -> Result<(PrintOptions, Option<String>), String> {
    let mut opts = PrintOptions::default();
    let mut path = None;

    for arg in args {
        if let Some(value) = arg.strip_prefix("--max-depth=") {
            let depth: usize = value
                .parse()
                .map_err(|_| format!("invalid --max-depth value {:?}", value))?;
            opts.max_depth = Some(depth);
        } else if let Some(value) = arg.strip_prefix("--collapse-under=") {
            let threshold: u64 = value
                .parse()
                .map_err(|_| format!("invalid --collapse-under value {:?}", value))?;
            opts.collapse_under = Some(threshold);
        } else if let Some(value) = arg.strip_prefix("--sort=") {
            opts.sort = match value {
                "size" => SortMode::Size,
                "name" => SortMode::Name,
                _ => return Err(format!("invalid --sort value {:?} (expected \"size\" or \"name\")", value)),
            };
        } else if let Some(flag) = arg.strip_prefix("--") {
            return Err(format!("unknown flag --{}", flag));
        } else if path.is_some() {
            return Err("only one input path may be given".to_string());
        } else {
            path = Some(arg.clone());
        }
    }

    Ok((opts, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_and_path_in_any_order() {
        let args: Vec<String> = vec!["--max-depth=2".to_string(), "usage.txt".to_string(), "--collapse-under=1024".to_string()];
        let (opts, path) = parse_args(&args).unwrap();
        assert_eq!(opts.max_depth, Some(2));
        assert_eq!(opts.collapse_under, Some(1024));
        assert_eq!(path, Some("usage.txt".to_string()));
    }

    #[test]
    fn rejects_unknown_flag() {
        let args: Vec<String> = vec!["--bogus".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn parses_sort_flag() {
        let args: Vec<String> = vec!["--sort=name".to_string()];
        let (opts, _) = parse_args(&args).unwrap();
        assert_eq!(opts.sort, SortMode::Name);
    }

    #[test]
    fn rejects_invalid_sort_value() {
        let args: Vec<String> = vec!["--sort=alphabetical".to_string()];
        assert!(parse_args(&args).is_err());
    }

    #[test]
    fn rejects_second_path() {
        let args: Vec<String> = vec!["a.txt".to_string(), "b.txt".to_string()];
        assert!(parse_args(&args).is_err());
    }
}
