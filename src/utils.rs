use crate::{
    syscalls::{close, fstat, openat, read, OpenFlags, OpenMode},
    CStr,
};
use alloc::vec::Vec;
use core::cmp::Ordering;

pub fn atoi(digits: &[u8]) -> u64 {
    let mut num = 0;
    for &c in digits {
        num = num * 10 + u64::from(c - b'0');
    }
    num
}

// This formatting code is modified from the code in itoa, to favor code size and compromises
// performance on formatting of large integers.

const U64_MAX_LEN: usize = 20;

const DEC_DIGITS_LUT: &[u8] = b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

pub struct Buffer {
    bytes: [u8; U64_MAX_LEN],
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            bytes: [0u8; U64_MAX_LEN],
        }
    }

    pub fn format(&mut self, n: u64) -> &[u8] {
        self.format_with_letter(n, 0)
    }

    pub fn format_with_letter(&mut self, mut n: u64, letter: u8) -> &[u8] {
        let buf = &mut self.bytes;
        let mut curr = buf.len();

        if n == 0 {
            buf[curr - 1] = b'0';
            return &buf[curr - 1..];
        }

        if letter != 0 {
            buf[curr - 1] = letter;
            curr -= 1;
        }

        while n >= 10 {
            let d1 = ((n % 100) << 1) as usize;
            n /= 100;
            curr -= 2;
            buf[curr..curr + 2].copy_from_slice(&DEC_DIGITS_LUT[d1..d1 + 2]);
        }

        if n > 0 {
            curr -= 1;
            buf[curr] = (n as u8) + b'0';
        }

        &buf[curr..]
    }

    /// precision: Number of digits after the dot
    pub fn format_f64_with_letter(&mut self, f: f64, mut precision: u64, letter: u8) -> &[u8] {
        let buf = &mut self.bytes;
        let mut curr = buf.len();

        if letter != 0 {
            buf[curr - 1] = letter;
            curr -= 1;
        }

        let mut n = (f * (precision as f64 * 10.0)) as u64;
        if n == 0 {
            buf[curr - 1] = b'0';
            return &buf[curr - 1..];
        }

        // Do the digits one by one, there won't be many because we humanized
        while n > 0 {
            let d1 = (n % 10) as u8;
            n /= 10;
            curr -= 1;
            buf[curr] = b'0' + d1;
            match precision.cmp(&1) {
                Ordering::Greater => precision -= 1,
                Ordering::Equal => {
                    curr -= 1;
                    buf[curr] = b'.';
                    precision = 0;
                }
                _ => {}
            }
        }

        &buf[curr..]
    }

    /// Takes a file size in bytes and returns it as a human friendly value.
    ///
    /// The size returned is the smallest rounded value in which this file will
    /// fit - i.e. it is a ceiling. A file of 2882, when divided by 1024 is 2.8144.
    /// It rounds to 2.8K but it won't fit in exactly 2.8K, so we display it as 2.9K.
    /// This matches what GNU ls does.
    ///
    /// Examples:
    /// - 2.1K
    /// - 142K
    /// - 16M
    /// - 4.8G
    pub fn humanize(&mut self, u: u64) -> &[u8] {
        const KILO: f64 = 1024.0;
        const MEGA: f64 = KILO * 1024.0;
        const GIGA: f64 = MEGA * 1024.0;
        const TERA: f64 = GIGA * 1024.0;

        let size = u as f64;
        let (divider, letter) = if size < KILO {
            (1.0, 0)
        } else if size < MEGA {
            (KILO, b'K')
        } else if size < GIGA {
            (MEGA, b'M')
        } else if size < TERA {
            (GIGA, b'G')
        } else {
            (TERA, b'T')
        };

        if divider == 1.0 {
            self.format(u)
        } else {
            let scaled = size / divider;
            // The stabilized f64::ceil is in 'std' not 'core', so use the intrinsic
            let int_rnd = unsafe { core::intrinsics::ceilf64(scaled) };
            if scaled >= 10.0 {
                // No decimal digit
                self.format_with_letter(int_rnd as u64, letter)
            } else {
                // Round to one decimal
                let h = unsafe { core::intrinsics::ceilf64(scaled * 10.0) } / 10.0;
                if h == int_rnd {
                    // decimal is 0
                    self.format_with_letter(int_rnd as u64, letter)
                } else {
                    // One decimal digit
                    self.format_f64_with_letter(h, 1, letter)
                }
            }
        }
    }
}

// This is significantly faster than using the libc implementation because our slices are usually
// very small, and this implementation emits very little code so it is very profitable to inline.
pub fn memcmp(aa: &[u8], bb: &[u8]) -> core::cmp::Ordering {
    for (a, b) in aa.iter().zip(bb.iter()) {
        if *a != *b {
            return a.cmp(b);
        }
    }
    aa.len().cmp(&bb.len())
}

pub fn fs_read(path: CStr<'_>) -> Result<Vec<u8>, crate::Error> {
    let fd = openat(libc::AT_FDCWD, path, OpenFlags::RDONLY, OpenMode::empty())?;
    let len = fstat(fd)?.st_size as usize;
    let mut contents = alloc::vec![0; len];
    let mut bytes_read = 0;
    while bytes_read < len {
        bytes_read += read(fd, &mut contents[bytes_read..])?;
    }
    close(fd)?;
    Ok(contents)
}
