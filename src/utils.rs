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
