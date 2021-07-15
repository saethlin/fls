use core::{ptr, slice};

pub fn atoi(digits: &[u8]) -> u64 {
    let mut num = 0;
    for &c in digits {
        num = num * 10 + (c - b'0') as u64;
    }
    num
}

// This formatting code is modified from the code in itoa, to favor code size and compromises
// performance on formatting of large integers.

const U64_MAX_LEN: usize = 20;

const DEC_DIGITS_LUT: &'static [u8] = b"\
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

    pub fn format(&mut self, n: u64) -> &[u8] {
        let buf = &mut self.bytes;
        let mut curr = buf.len() as isize;
        let buf_ptr = buf.as_mut_ptr();
        let lut_ptr = DEC_DIGITS_LUT.as_ptr();

        unsafe {
            let mut n = n as isize; // possibly reduce 64bit math

            // decode 2 more chars, if > 2 chars
            while n >= 100 {
                let d1 = (n % 100) << 1;
                n /= 100;
                curr -= 2;
                ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
            }

            // decode last 1 or 2 chars
            if n < 10 {
                curr -= 1;
                *buf_ptr.offset(curr) = (n as u8) + b'0';
            } else {
                let d1 = n << 1;
                curr -= 2;
                ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
            }
        }

        let len = buf.len() - curr as usize;
        unsafe { slice::from_raw_parts(buf_ptr.offset(curr), len) }
    }
}
