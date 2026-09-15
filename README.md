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

Plain `du -a` (no `-b`) doesn't report bytes — it reports a count of
512-byte blocks, rounded up per file, on both GNU and BSD du. Pass
`--size-unit=blocks` to tell du-pretty to scale those counts up to bytes
before printing:

```
$ du -a /var | du-pretty --size-unit=blocks
```

The default, `--size-unit=bytes`, assumes the input is already in bytes,
matching `du -ab`.

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

## JSON output

`--json` prints the same tree as a JSON array of nodes instead of indented
text, for piping into other tools:

```
$ du-pretty --json --max-depth=1 usage.txt
[{"path":"/var","name":"var","size":8192,"children":[{"path":"/var/cache","name":"cache","size":8192,"children":[]},{"path":"/var/log","name":"log","size":2048,"children":[],"hidden":{"count":2,"size":2048}}]}]
```

Each node has `path`, `name` (basename), `size`, and `children`. `--max-depth`
and `--collapse-under` apply the same as in text mode; whatever they'd hide
shows up as a `hidden` field (`count` and total `size`) on the node whose
descendants were folded away, instead of being dropped from the output.

## Status

Parsing and printing both work end to end: both of `du`'s size conventions
(bytes and 512-byte blocks) are understood, output can be trimmed and
sorted, and it can be printed as text or JSON. `du -ab`'s flat format
carries no inode numbers, so there's currently no way to detect when two
entries are actually the same hard-linked file - see below.

## Roadmap

- detect hard-linked files and flag them so their size isn't double-counted
  once an input format that carries inode numbers is available
