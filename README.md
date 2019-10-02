Attempting to write a POSIX-compliant `ls` in Rust that's faster and prettier than GNU's.

- [x] -A
- [x] -C
- [x] -F
- [x] -H
- [x] -L
- [ ] -R
- [x] -S
- [x] -a
- [ ] -c
- [x] -d
- [x] -f
- [ ] -g
- [ ] -i
- [ ] -k
- [ ] -l
- [ ] -m
- [ ] -n
- [ ] -o
- [x] -p
- [ ] -q
- [x] -r
- [ ] -s
- [ ] -t
- [ ] -u
- [ ] -x
- [ ] -1

## Motivation

I do a lot of work on an HPC system, where `lstat` is very slow. Both GNU ls and [exa](https://github.com/ogham/exa) use `lstat` to select colors for their output, but all the information needed to produce colored output can be derived from `getdents` and `faccessat` (with a modern kernel and mainstream filesystem).
This project started as an experiment to see how much faster than GNU ls and exa an ls-like program could be, but it turns out that in less than 2 weeks of spare time one can implement a competent ls-like program so that's what this is now. I currently use this program in lieu of the system ls.

There are a lot of algorithms in exa that are much slower than they need to be, but some of those could be addressed over with a good PR. There is a deeper problem that its entire architecture is performance-hostile (exa _loves_ allocating little strings); exa would probably need something equivalent to a full rewrite to compete with GNU ls in speed.
