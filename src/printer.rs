// Turns the flat entry list back into an indented tree with human-readable
// sizes, biggest child first, which is the layout you actually want when
// you're hunting for what's eating a disk.

use crate::parser::{parent_of, Entry};
use std::collections::HashMap;

pub fn print_tree(entries: &[Entry]) -> String {
    let known: std::collections::HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();

    let mut children: HashMap<Option<&str>, Vec<&Entry>> = HashMap::new();
    let mut roots: Vec<&Entry> = Vec::new();

    for entry in entries {
        match parent_of(&entry.path) {
            Some(parent) if known.contains(parent) => {
                children.entry(Some(parent)).or_default().push(entry);
            }
            _ => roots.push(entry),
        }
    }

    for list in children.values_mut() {
        list.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    }
    roots.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));

    let mut out = String::new();
    for root in &roots {
        write_entry(root, &children, 0, &mut out);
    }
    out
}

fn write_entry(entry: &Entry, children: &HashMap<Option<&str>, Vec<&Entry>>, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    out.push_str(&format!(
        "{}{:>10}  {}\n",
        indent,
        human_size(entry.size),
        basename(&entry.path)
    ));

    if let Some(kids) = children.get(&Some(entry.path.as_str())) {
        for child in kids {
            write_entry(child, children, depth + 1, out);
        }
    }
}

fn basename(path: &str) -> &str {
    if path == "/" {
        return "/";
    }
    match path.rsplit_once('/') {
        Some((_, name)) if !name.is_empty() => name,
        _ => path,
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{}{}", bytes, UNITS[0]);
    }
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.1}{}", size, UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_units() {
        assert_eq!(human_size(512), "512B");
        assert_eq!(human_size(2048), "2.0KiB");
        assert_eq!(human_size(1024 * 1024 * 3), "3.0MiB");
    }

    #[test]
    fn basename_handles_root() {
        assert_eq!(basename("/"), "/");
        assert_eq!(basename("/var/log"), "log");
    }
}
