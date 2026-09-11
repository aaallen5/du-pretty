// Turns the flat entry list back into an indented tree with human-readable
// sizes, biggest child first, which is the layout you actually want when
// you're hunting for what's eating a disk.

use crate::parser::{parent_of, Entry};
use std::collections::HashMap;

type ChildMap<'a> = HashMap<Option<&'a str>, Vec<&'a Entry>>;

// max_depth counts from the roots (depth 0): a root's direct children are
// depth 1, and once a printed entry sits at max_depth its own children are
// summarized rather than printed. collapse_under hides individual entries
// smaller than the threshold, folding them into one summary line per
// directory instead of drowning the interesting entries in noise.
#[derive(Debug, Default, Clone, Copy)]
pub struct PrintOptions {
    pub max_depth: Option<usize>,
    pub collapse_under: Option<u64>,
    pub sort: SortMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Size,
    Name,
}

impl Default for SortMode {
    fn default() -> Self {
        SortMode::Size
    }
}

fn sort_siblings(list: &mut [&Entry], mode: SortMode) {
    match mode {
        SortMode::Size => list.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path))),
        SortMode::Name => list.sort_by(|a, b| basename(&a.path).cmp(basename(&b.path))),
    }
}

pub fn print_tree(entries: &[Entry], opts: &PrintOptions) -> String {
    let known: std::collections::HashSet<&str> = entries.iter().map(|e| e.path.as_str()).collect();

    let mut children: ChildMap = HashMap::new();
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
        sort_siblings(list, opts.sort);
    }
    sort_siblings(&mut roots, opts.sort);

    let mut out = String::new();
    for root in &roots {
        write_entry(root, &children, 0, opts, &mut out);
    }
    out
}

fn write_entry(entry: &Entry, children: &ChildMap, depth: usize, opts: &PrintOptions, out: &mut String) {
    let indent = "  ".repeat(depth);
    out.push_str(&format!(
        "{}{:>10}  {}\n",
        indent,
        human_size(entry.size),
        basename(&entry.path)
    ));

    let Some(kids) = children.get(&Some(entry.path.as_str())) else {
        return;
    };

    if opts.max_depth.is_some_and(|max| depth >= max) {
        let hidden_size: u64 = kids.iter().map(|k| k.size).sum();
        let hidden_count = kids.iter().map(|k| 1 + count_descendants(&k.path, children)).sum();
        write_other_line(hidden_count, hidden_size, depth + 1, out);
        return;
    }

    let (shown, other) = split_for_collapse(kids, opts.collapse_under);
    for child in &shown {
        write_entry(child, children, depth + 1, opts, out);
    }
    if let Some((count, size)) = other {
        write_other_line(count, size, depth + 1, out);
    }
}

fn count_descendants(path: &str, children: &ChildMap) -> usize {
    match children.get(&Some(path)) {
        None => 0,
        Some(kids) => kids.iter().map(|k| 1 + count_descendants(&k.path, children)).sum(),
    }
}

// kids arrives already sorted per opts.sort; partitioning by size preserves
// that relative order within `shown` regardless of which sort mode produced
// it, so this never needs a re-sort afterward.
fn split_for_collapse<'a>(kids: &[&'a Entry], threshold: Option<u64>) -> (Vec<&'a Entry>, Option<(usize, u64)>) {
    let Some(threshold) = threshold else {
        return (kids.to_vec(), None);
    };

    let mut shown = Vec::new();
    let mut other_count = 0usize;
    let mut other_size = 0u64;
    for k in kids {
        if k.size < threshold {
            other_count += 1;
            other_size += k.size;
        } else {
            shown.push(*k);
        }
    }

    let other = if other_count > 0 { Some((other_count, other_size)) } else { None };
    (shown, other)
}

fn write_other_line(count: usize, size: u64, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let noun = if count == 1 { "entry" } else { "entries" };
    out.push_str(&format!(
        "{}{:>10}  ... {} more {}\n",
        indent,
        human_size(size),
        count,
        noun
    ));
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

    fn sample_entries() -> Vec<Entry> {
        vec![
            Entry { size: 4096, path: "/var".to_string(), line: 1 },
            Entry { size: 3072, path: "/var/log".to_string(), line: 2 },
            Entry { size: 2048, path: "/var/log/syslog".to_string(), line: 3 },
            Entry { size: 1, path: "/var/log/auth.log".to_string(), line: 4 },
            Entry { size: 1, path: "/var/log/kern.log".to_string(), line: 5 },
            Entry { size: 1024, path: "/var/cache".to_string(), line: 6 },
        ]
    }

    #[test]
    fn no_options_prints_full_tree() {
        let entries = sample_entries();
        let out = print_tree(&entries, &PrintOptions::default());
        assert_eq!(out.lines().count(), entries.len());
    }

    #[test]
    fn max_depth_collapses_deeper_entries_into_a_summary() {
        let entries = sample_entries();
        let opts = PrintOptions { max_depth: Some(1), collapse_under: None, ..Default::default() };
        let out = print_tree(&entries, &opts);
        // roots + their direct children (var, cache, log) print in full, the
        // three entries under /var/log fold into one "more entries" line.
        assert_eq!(out.lines().count(), 4);
        assert!(out.contains("2.0KiB  ... 3 more entries"));
    }

    #[test]
    fn collapse_under_folds_small_siblings_together() {
        let entries = sample_entries();
        let opts = PrintOptions { max_depth: None, collapse_under: Some(1024), ..Default::default() };
        let out = print_tree(&entries, &opts);
        assert!(out.contains("2B  ... 2 more entries"));
        assert!(!out.contains("auth.log"));
        assert!(!out.contains("kern.log"));
        assert!(out.contains("syslog"));
    }

    #[test]
    fn sort_by_name_orders_siblings_alphabetically() {
        let entries = sample_entries();
        let opts = PrintOptions { sort: SortMode::Name, ..Default::default() };
        let out = print_tree(&entries, &opts);
        let auth_pos = out.find("auth.log").unwrap();
        let kern_pos = out.find("kern.log").unwrap();
        let syslog_pos = out.find("syslog").unwrap();
        assert!(auth_pos < kern_pos);
        assert!(kern_pos < syslog_pos);
    }
}
