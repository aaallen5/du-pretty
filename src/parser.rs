// Parses the flat "<size>\t<path>" lines that `du` (and du -a, du -b) emit,
// and checks that the result actually forms a tree before we trust it.

use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Entry {
    pub size: u64,
    pub path: String,
    pub line: usize,
}

#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

pub fn parse(input: &str) -> Result<Vec<Entry>, ParseError> {
    let mut entries = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let size_field = parts.next().unwrap_or("");
        let path_field = parts.next().unwrap_or("").trim_start();

        if size_field.is_empty() {
            return Err(ParseError {
                line: line_no,
                message: "missing size field".to_string(),
            });
        }
        let size: u64 = size_field.parse().map_err(|_| ParseError {
            line: line_no,
            message: format!("size {:?} is not a non-negative integer", size_field),
        })?;

        if path_field.is_empty() {
            return Err(ParseError {
                line: line_no,
                message: "missing path field".to_string(),
            });
        }

        if path_field.len() > 1 && path_field.ends_with('/') {
            return Err(ParseError {
                line: line_no,
                message: format!("path {:?} has a trailing slash", path_field),
            });
        }

        if !seen_paths.insert(path_field.to_string()) {
            return Err(ParseError {
                line: line_no,
                message: format!("duplicate path {:?}", path_field),
            });
        }

        entries.push(Entry {
            size,
            path: path_field.to_string(),
            line: line_no,
        });
    }

    validate_tree(&entries)?;
    Ok(entries)
}

// Every entry except a root must have its parent directory present as its
// own entry somewhere in the report; otherwise the tree has a hole in it
// and the pretty printer would have nowhere to hang the entry.
pub(crate) fn parent_of(path: &str) -> Option<&str> {
    if path == "/" || path == "." {
        return None;
    }
    match path.rfind('/') {
        None => None,
        Some(0) => Some("/"),
        Some(idx) => Some(&path[..idx]),
    }
}

fn validate_tree(entries: &[Entry]) -> Result<(), ParseError> {
    let known: HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    for entry in entries {
        if let Some(parent) = parent_of(&entry.path) {
            if !known.contains(parent) {
                return Err(ParseError {
                    line: entry.line,
                    message: format!(
                        "path {:?} has no matching parent entry {:?}",
                        entry.path, parent
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_report() {
        let input = "4096\t/var\n2048\t/var/log\n";
        let entries = parse(input).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "/var/log");
    }

    #[test]
    fn rejects_missing_parent() {
        let input = "10\t/var/log/syslog\n";
        assert!(parse(input).is_err());
    }

    #[test]
    fn rejects_duplicate_path() {
        let input = "1\t/a\n2\t/a\n";
        assert!(parse(input).is_err());
    }
}
