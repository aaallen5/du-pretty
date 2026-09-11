# du-pretty

`du -a` dumps a flat list of byte counts and paths in whatever order the
filesystem walk happened to visit them. Reading that list to find out what's
actually eating your disk means doing the tree reconstruction and the
byte-to-MiB conversion in your head. du-pretty does that part for you: it
parses the flat report, checks that it's actually internally consistent (no
duplicate paths, no entry whose parent directory is missing from the
report), and prints it back as an indented tree sorted biggest-first with
human-readable sizes.

## Input format

One entry per line, tab- or space-separated, size in bytes first:

```
4096	/var
2048	/var/log
1024	/var/log/syslog
1024	/var/log/auth.log
8192	/var/cache
```

This is exactly what `du -ab` produces. Every path other than a root must
have its parent directory present as its own line somewhere in the report —
that's the invariant the parser checks before handing anything to the
printer.

## Usage

```
$ du -ab /var | du-pretty
```

or from a saved report:

```
$ du -ab /var > usage.txt
$ du-pretty usage.txt
   8.0KiB  var
   8.0KiB    cache
   2.0KiB    log
   1.0KiB      syslog
   1.0KiB      auth.log
```

If a line is malformed or the tree doesn't add up, du-pretty reports the
line number and exits non-zero instead of guessing:

```
$ printf '10\t/var/log/syslog\n' | du-pretty
du-pretty: line 1: path "/var/log/syslog" has no matching parent entry "/var/log"
```

## Trimming large trees

Two flags keep a wide or deep report readable. Both fold the entries they
hide into a single `... N more entries` summary line per directory, sized
to the total they represent, rather than dropping them silently:

```
$ du-pretty --max-depth=1 usage.txt
   8.0KiB  var
   8.0KiB    cache
   2.0KiB    log
     2.0KiB      ... 2 more entries

$ du-pretty --collapse-under=2048 usage.txt
   8.0KiB  var
   8.0KiB    cache
   2.0KiB    log
   1.0KiB      syslog
     1.0KiB      ... 1 more entries
```

`--max-depth=N` stops descending past depth N from the root. `--collapse-under=BYTES`
hides individual entries smaller than the threshold within each directory.
They can be combined.

## Sorting

By default siblings print biggest-first. `--sort=name` prints them
alphabetically by basename instead:

```
$ du-pretty --sort=name usage.txt
   8.0KiB  var
   1.0KiB    auth.log
   8.0KiB    cache
   2.0KiB    log
```

## Status

Early skeleton: parsing and printing both work end to end, but there's no
handling yet for symlinks, hard-link double-counting, or `du`'s block-size
vs byte-size ambiguity. See below.

## Roadmap

- read multiple `du` block-size conventions (512-byte blocks vs `-b` bytes)
- detect and flag hard-linked files so their size isn't double-counted
- `--json` output mode for piping into other tools
