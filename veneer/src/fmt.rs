// This formatting code is modified from the code in itoa, to favor code size and compromises
// performance on formatting of large integers.

const U64_MAX_LEN: usize = 20;

const DEC_DIGITS_LUT: &[u8] = b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

#[derive(Default)]
pub struct Buffer {
    buf: [u8; U64_MAX_LEN],
}

impl Buffer {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn format(&mut self, mut n: u64) -> &[u8] {
        let mut curr = self.buf.len();
        let buf = &mut self.buf;

        if n == 0 {
            buf[buf.len() - 1] = b'0';
            return &buf[buf.len() - 1..];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all() {
        for i in 0..(u16::MAX as u64) {
            assert_eq!(
                itoa::Buffer::new().format(i).as_bytes(),
                Buffer::new().format(i)
            );
        }
    }
}
