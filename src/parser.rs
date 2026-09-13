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

// `du -ab` reports bytes directly. Plain `du -a` (no -b, no -k) reports
// counts of 512-byte blocks on both GNU and BSD du, rounded up to the block
// that holds each file - that's SizeUnit::Blocks512.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    Bytes,
    Blocks512,
}

impl Default for SizeUnit {
    fn default() -> Self {
        SizeUnit::Bytes
    }
}

impl SizeUnit {
    fn scale(self, count: u64) -> Result<u64, &'static str> {
        match self {
            SizeUnit::Bytes => Ok(count),
            SizeUnit::Blocks512 => count.checked_mul(512).ok_or("size overflows after applying block size"),
        }
    }
}

pub fn parse(input: &str, unit: SizeUnit) -> Result<Vec<Entry>, ParseError> {
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
        let raw_size: u64 = size_field.parse().map_err(|_| ParseError {
            line: line_no,
            message: format!("size {:?} is not a non-negative integer", size_field),
        })?;
        let size = unit.scale(raw_size).map_err(|msg| ParseError {
            line: line_no,
            message: format!("{} ({:?})", msg, size_field),
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
        let entries = parse(input, SizeUnit::Bytes).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].path, "/var/log");
    }

    #[test]
    fn rejects_missing_parent() {
        let input = "10\t/var/log/syslog\n";
        assert!(parse(input, SizeUnit::Bytes).is_err());
    }

    #[test]
    fn rejects_duplicate_path() {
        let input = "1\t/a\n2\t/a\n";
        assert!(parse(input, SizeUnit::Bytes).is_err());
    }

    #[test]
    fn blocks512_scales_sizes_up_to_bytes() {
        let input = "8\t/var\n1\t/var/log\n";
        let entries = parse(input, SizeUnit::Blocks512).unwrap();
        assert_eq!(entries[0].size, 4096);
        assert_eq!(entries[1].size, 512);
    }

    #[test]
    fn blocks512_rejects_overflowing_size() {
        let input = "18446744073709551615\t/var\n";
        assert!(parse(input, SizeUnit::Blocks512).is_err());
    }
}
