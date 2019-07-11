## Motivation

I do a lot of work on an HPC system, where `lstat` is very slow. Both GNU ls and [exa](https://github.com/ogham/exa) use `lstat` to select colors for their output. But all the information needed to produce colored output can be derived from `getdents` and `faccessat`. This project started out as an experiment to see how much faster than GNU ls and exa an ls-like program could be, but it turns out that in less than 2 weeks of spare time one can implement a competent ls-like program so that's what this is now.

There are a lot of algorithms in exa that are much slower than they need to be, but some of those could be adressed over with a good PR. There is a deeper problem that its entire architecture is performance-hostile (exa _loves_ allocating little strings); exa would probably need something equivlane to a full rewrite to compete with GNU ls in speed.

## Aims

This project provides a for-humans style of ls similar to exa while being faster than both exa and GNU ls. This project is currently faster with colorful output and a more sophistocated sorting order than GNU ls is without colors and lexicographic sorting, and I intend to keep it that way.

## Features/Status

Basic grid output with exa-adjacent coloring is implemented, though I haven't gotten around to adding support for all the varying file names it detects and highlights for.

`-a` is implemented in the style of exa.

`-l` is mostly implemented, a few permissions indicators are missing.
