use crate::{
    syscalls::{close, fstat, open, read, OpenFlags, OpenMode},
    CStr,
};
use alloc::vec::Vec;
use core::{ptr, slice};

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
    #[inline]
    pub fn new() -> Buffer {
        Buffer {
            bytes: [0u8; U64_MAX_LEN],
        }
    }

    pub fn format(&mut self, mut n: u64) -> &[u8] {
        let buf = &mut self.bytes;
        let mut curr = buf.len();
        let buf_ptr = buf.as_mut_ptr();
        let lut_ptr = DEC_DIGITS_LUT.as_ptr();

        unsafe {
            // decode 2 more chars, if > 2 chars
            while n >= 100 {
                let d1 = (n % 100) << 1;
                n /= 100;
                curr -= 2;
                ptr::copy_nonoverlapping(lut_ptr.add(d1 as usize), buf_ptr.add(curr), 2);
            }

            // decode last 1 or 2 chars
            if n < 10 {
                curr -= 1;
                *buf_ptr.add(curr) = (n as u8) + b'0';
            } else {
                let d1 = n << 1;
                curr -= 2;
                ptr::copy_nonoverlapping(lut_ptr.add(d1 as usize), buf_ptr.add(curr), 2);
            }
        }

        let len = buf.len() - curr;
        unsafe { slice::from_raw_parts(buf_ptr.add(curr), len) }
    }
}

pub fn memcpy(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    let len = src.len();
    unsafe {
        let end = src.as_ptr().add(len) as *const u32;
        let mut dst = dst.as_mut_ptr() as *mut u32;
        let mut src = src.as_ptr() as *const u32;

        while end > src {
            dst.write_unaligned(src.read_unaligned());
            dst = dst.add(1);
            src = src.add(1);
        }

        let end = end.cast::<u8>();
        let mut dst = dst.cast::<u8>();
        let mut src = src.cast::<u8>();
        while end > src {
            *dst = *src;
            dst = dst.add(1);
            src = src.add(1);
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
    return aa.len().cmp(&bb.len());
}

pub fn fs_read(path: CStr<'_>) -> Result<Vec<u8>, crate::Error> {
    let fd = open(path, OpenFlags::RDONLY, OpenMode::empty())?;
    let len = fstat(fd)?.st_size as usize;
    let mut contents = alloc::vec![0; len];
    let mut bytes_read = 0;
    while bytes_read < len {
        bytes_read += read(fd, &mut contents[bytes_read..])?;
    }
    close(fd)?;
    Ok(contents)
}
