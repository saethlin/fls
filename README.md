## fls
A nearly POSIX-compliant `ls` that's smaller, faster, and prettier than GNU's.

[exa](https://github.com/ogham/exa) and [lsd](https://github.com/Peltoche/lsd) are both great `ls`-like Rust programs, but they're slower than the system `ls` and about 10x the code size. Plus you can't actually replace your `ls` with one of them, because some software relies on parsing the output of `ls`. But even as a user experience improvement, I think other projects tell the wrong story; modern software does not need to be larger or slower. It can be smaller and faster if we put in the effort.

| ls -R / --color=never > /dev/null  | Wall time (s) |
| ------------- | ------------- |
| fls | 0.66 |
| GNU ls  | 1.22  |
| exa  | 3.61 |
| lsd  | >1000  |

## But How?

`fls` addresses code size by being `#![no_std]`, which is important not because the standard library is in general large, but because the standard library's panic runtime is massive. The rest of the code size was trimmed down mostly by running the excellent tool [`cargo bloat`](https://crates.io/crates/cargo-bloat) to identify places to replace generics with runtime dispatch.

In terms of speed, `fls` is faster because it doesn't use the POSIX interfaces for listing files. We directly call `getdents64` and parse the output, instead of dealing with all the calls and heap allocation of `read_dir`. And in addition to this, we get access to the directory entry type member, which lets us omit a number of `stat` calls, which can be expensive relative to other fs syscalls.

## `--color=auto`

`fls` has the same interpretation as GNU ls for `--color=always` and `--color=never`, but under `--color=auto`, `fls` will _only_ apply colors based on file extension and the information available from `getdents64`, which is optional. Thus, the coloring of `fls --color=auto` is unpredictable, but you get _some_ coloring of output without any expensive `stat` calls. `fls` was originally developed when I was working a lot on an HPC filesystem, and `ls --color=always` on large directories could take seconds to minutes. `fls --color=auto` provides the same colors in those directories, in the blink of an eye.

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
