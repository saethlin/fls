Attempting to write a POSIX-compliant `ls` in Rust that's faster and prettier than GNU's.

[exa](https://github.com/ogham/exa) and [lsd](https://github.com/Peltoche/lsd) are both great `ls`-like Rust programs, but they're slower than what they intend to replace. This project is a demonstration that we can make things better and faster at the same time.

- [x] -A do not list implied `.` and `..`
- [x] -C list entries in columns
- [x] -F append an indicator to entries
- [x] -H follow symlinks when provided on the command line
- [x] -L always follow symlinks
- [ ] -R recurse into subdirectories
- [x] -S sort by size
- [x] -a do not ignore entries whose names begin with `.`
- [x] -c sort by ctime
- [ ] -d list directories themselves, not their contents
- [x] -f do not sort
- [ ] -g long format but without owner
- [ ] -i print each entrie's inode
- [ ] -k treat block size to 1024 bytes
- [ ] -l long format
- [x] -m single row, separated by `, `
- [ ] -n long format but list uid and gid instead of names
- [ ] -o long format but without groups
- [x] -p append an indicator to directories
- [ ] -q replace non-printable characters with `?`
- [x] -r reverse sorting order
- [ ] -s print size of each file in blocks
- [x] -t sort by modification time
- [x] -u sort by access time
- [ ] -x sort entries across rows
- [x] -1 list one entry per line
