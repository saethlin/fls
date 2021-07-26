use crate::{
    syscalls::{close, fstat, openat, read, OpenFlags, OpenMode},
    CStr,
};
use alloc::vec::Vec;

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

    pub fn format(&mut self, mut n: u64) -> &[u8] {
        let buf = &mut self.bytes;
        let mut curr = buf.len();

        if n == 0 {
            buf[curr - 1] = b'0';
            return &buf[curr - 1..];
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
