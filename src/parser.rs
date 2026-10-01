// Parses the flat "<size>\t<path>" lines that `du` (and du -a, du -b) emit,
// and checks that the result actually forms a tree before we trust it.

use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone)]
pub struct Entry {
    pub size: u64,
    pub path: String,
    pub line: usize,
    // Path of the earlier entry sharing this entry's inode, if any.
    pub link_of: Option<String>,
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

// `du` itself never prints inode numbers, so hard links can only be spotted
// when the report comes from something that does, e.g.
// `find /var -printf '%i %s %p\n'`. WithInodes expects that
// "<inode> <size> <path>" shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputFormat {
    #[default]
    Plain,
    WithInodes,
}

pub fn parse(input: &str, unit: SizeUnit) -> Result<Vec<Entry>, ParseError> {
    parse_with(input, unit, InputFormat::Plain)
}

pub fn parse_with(input: &str, unit: SizeUnit, format: InputFormat) -> Result<Vec<Entry>, ParseError> {
    let mut entries = Vec::new();
    let mut seen_paths: HashSet<String> = HashSet::new();
    let mut first_by_inode: HashMap<u64, String> = HashMap::new();

    for (idx, raw_line) in input.lines().enumerate() {
        let line_no = idx + 1;
        let mut line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        let mut inode = None;
        if format == InputFormat::WithInodes {
            let mut head = line.trim_start().splitn(2, char::is_whitespace);
            let inode_field = head.next().unwrap_or("");
            inode = Some(inode_field.parse::<u64>().map_err(|_| ParseError {
                line: line_no,
                message: format!("inode {:?} is not a non-negative integer", inode_field),
            })?);
            line = head.next().unwrap_or("").trim_start();
            if line.is_empty() {
                return Err(ParseError {
                    line: line_no,
                    message: "missing size field".to_string(),
                });
            }
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

        // Only the first path seen for an inode is the "real" one; later
        // paths with the same inode are links to it.
        let link_of = inode.and_then(|ino| match first_by_inode.get(&ino) {
            Some(first) => Some(first.clone()),
            None => {
                first_by_inode.insert(ino, path_field.to_string());
                None
            }
        });

        entries.push(Entry {
            size,
            path: path_field.to_string(),
            line: line_no,
            link_of,
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

    #[test]
    fn plain_format_never_marks_links() {
        let entries = parse("4096\t/d\n10\t/d/a\n10\t/d/b\n", SizeUnit::Bytes).unwrap();
        assert!(entries.iter().all(|e| e.link_of.is_none()));
    }

    #[test]
    fn inode_format_flags_later_paths_sharing_an_inode() {
        let input = "1 4096 /d\n7 10 /d/a\n8 5 /d/c\n7 10 /d/b\n";
        let entries = parse_with(input, SizeUnit::Bytes, InputFormat::WithInodes).unwrap();
        assert_eq!(entries[1].link_of, None);
        assert_eq!(entries[2].link_of, None);
        assert_eq!(entries[3].link_of.as_deref(), Some("/d/a"));
    }

    #[test]
    fn inode_format_rejects_non_numeric_inode() {
        let err = parse_with("x 10 /d\n", SizeUnit::Bytes, InputFormat::WithInodes).unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.message.contains("inode"));
    }

    #[test]
    fn inode_format_rejects_line_with_only_an_inode() {
        assert!(parse_with("7\n", SizeUnit::Bytes, InputFormat::WithInodes).is_err());
    }
}
