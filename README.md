## fls
A nearly-POSIX-compliant and libc-less `ls` that's smaller, faster, and prettier than GNU's<sup>1</sup>.

[exa](https://github.com/ogham/exa) and [lsd](https://github.com/Peltoche/lsd) are both great `ls`-like Rust programs, but they're slower than the system `ls` and about 10x the code size. Plus you can't actually replace your `ls` with one of them, because some software relies on parsing the output of `ls`. But even as a user experience improvement, I think other projects tell the wrong story; modern software does not need to be larger or slower.

<sup>1</sup>I don't mean to rag on GNU's `ls`, but as far as I can tell it's the closest thing along the metrics I value.

## Crude benchmarks
|          | --color=never -R / > /dev/null | --color=always -R /  | --color=auto ~ | --color=auto -l ~ |
| ---------| ------------------------------ | -------------------- | -------------- | ----------------- |
| `fls`    | 0.66 s                         | 2.32 s               | 0.16 ms        | 0.30 ms           |
| GNU `ls` | 1.22 s                         | 4.37 s               | 0.38 ms        | 2.30 ms           |
| `exa`    | 3.61 s                         | 63.7 s <sup>3</sup>  | 0.78 ms        | 3.30 ms <sup>4</sup> |
| `lsd`    | ???<sup>2</sup>                | ???<sup>2</sup>      | 36.5 ms        | 36.8 ms           |

These do not cover all reasonable combinations of options, but if you can find a combination of flags for which `fls` is slower than any alternatives, please open an issue.

<sup>2</sup>`lsd` doesn't detect symlink cycles and thus runs indefinitely on `-R /`.<br/>
<sup>3</sup>I have some large directories of fuzzing corpora; from running `perf top` as I was collecting this data, I see `exa` spends most of its time in `term_grid::Grid::column_widths`. I suspect its grid layout algorithm is quadratic.<br/>
<sup>4</sup>In all cases I report wall time; this is the only case where CPU time is significantly different. `exa`'s CPU time is ~2.2x this value.<br/>

## "libc-less"

By default `fls` does not link to anything. That's right, `fls` is smaller than GNU's `ls` implementation, even though GNU's gets to cheat by dynamically linking to a C standard library.

## smaller _and_ faster?

The biggest impact on code size is `#![no_std]`, because the standard library's runtime is relatively large. Most individual components of the standard library are a totally reasonable size, but the code for generating backtraces is huge and as far as I can tell `#![no_std]` is the only way to get rid of it. The rest of the code size was trimmed down mostly by running the excellent tool [`cargo bloat`](https://crates.io/crates/cargo-bloat) to identify places to replace generics with runtime dispatch, and just manually reviewing the code to factor out repeated code patterns.

In terms of speed, `fls` is _probably_ faster than GNU's `ls` because it doesn't use the POSIX interfaces for listing files. We directly call `getdents64` and parse the output, instead of juggling calls to `read_dir`. And since we're calling `getdents64`, we get access to the optional directory entry type information, which usually lets us omit a number of `stat` calls, which can be expensive relative to other filesystem syscalls.
I say _probably_ because `fls` has always been faster than GNU's `ls`. The original goal was just to use `getdents64` directly (see below), and as soon as I had a working prototype, it was faster than the competition.

## `--color=auto`

`fls` has the same interpretation as GNU ls for `--color=always` and `--color=never`, but under `--color=auto`, `fls` will _only_ apply colors based on file extension and the information available from `getdents64`, which is optional. Thus, the coloring of `fls --color=auto` is unpredictable, but you get _some_ coloring of output without any expensive `stat` calls. `fls` was originally developed when my dev environment was a compute node with an HPC filesystem, and `ls --color=always` on large directories could take seconds to minutes. `fls --color=auto` provides the same colors in those directories, in the blink of an eye. Thus, `--color=auto` is the assumed if no arguments are provided and stdout is a terminal.

## Sorting

In addition different color behavior, `fls` also 

## POSIX features:

- [x] -A do not list implied `.` and `..`
- [x] -C list entries in columns
- [x] -F append an indicator to entries
- [x] -H follow symlinks when provided on the command line
- [x] -L always follow symlinks
- [x] -R recurse into subdirectories
- [x] -S sort by size
- [x] -a do not ignore entries whose names begin with `.`
- [x] -c sort by ctime
- [x] -d list directories themselves, not their contents
- [x] -f do not sort
- [x] -g long format but without owner
- [x] -i print each entry's inode
- [ ] -k pretend block size is 1024 bytes
- [x] -l long format
- [x] -m single row, separated by `, `
- [x] -n long format but list uid and gid instead of names
- [x] -o long format but without groups
- [x] -p append an indicator to directories
- [ ] -q replace non-printable characters with `?`
- [x] -r reverse sorting order
- [x] -s print size of each file in blocks
- [x] -t sort by modification time
- [x] -u sort by access time
- [ ] -x sort entries across rows
- [x] -1 list one entry per line
