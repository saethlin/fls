Essentially, a replacement for the Rust standard library on Linux.

The Rust standard library makes tradeoffs in both API and implementation which are generally good but are inappropriate for some uses. This library offers an alternative perspective. In particular, it aims for:

* No linkage against a libc
* A minimum of unsafe code outside of that required to write syscall wrappers
* The lowest runtime overhead possible, even where that makes interfaces awkward

These motivations primarily come from my experience trying to implement a POSIX ls that isn't significantly larger or slower than GNU's ls. For small programs, the accidental complexity of combining Rust's standard library with a libc implementation becomes the dominant contributor of both code size and execution speed.
