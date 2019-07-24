use crate::Writable;
use core::cmp::Ordering;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
pub struct CStr<'a> {
    bytes: &'a [u8],
}

impl<'a> CStr<'a> {
    /// Requires that the passed-in pointer be a valid pointer to a null-terminated array of c_char
    pub unsafe fn from_ptr(ptr: *const u8) -> CStr<'a> {
        CStr {
            bytes: core::slice::from_raw_parts(ptr, libc::strlen(ptr as *const i8) + 1),
        }
    }

    pub fn from_bytes(bytes: &'a [u8]) -> CStr<'a> {
        assert!(
            bytes.last() == Some(&0),
            "attempted to construct a CStr from a slice without a null terminator"
        );
        CStr { bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.bytes.len() - 1]
    }

    pub fn get(&self, i: usize) -> Option<&u8> {
        self.bytes[..self.bytes.len() - 1].get(i)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    pub fn len_utf8(&self) -> usize {
        core::str::from_utf8(&self.bytes[..self.bytes.len() - 1])
            .map(|s| s.graphemes(false).count())
            .unwrap_or_else(|_| self.bytes.len() - 1)
    }

    pub fn vercmp(&self, s2_cstr: CStr) -> Ordering {
        let s1 = self.bytes;
        let s2 = s2_cstr.bytes;
        let mut s1_pos: usize = 0;
        let mut s2_pos: usize = 0;

        while s1_pos < s1.len() || s2_pos < s2.len() {
            let mut first_diff = Ordering::Equal;
            while (s1_pos < s1.len() && !s1.digit_at(s1_pos))
                || (s2_pos < s2.len() && !s2.digit_at(s2_pos))
            {
                let s1_c = s1.get(s1_pos).map(u8::to_ascii_lowercase);
                let s2_c = s2.get(s2_pos).map(u8::to_ascii_lowercase);
                if s1_c != s2_c {
                    return s1_c.cmp(&s2_c);
                }
                s1_pos += 1;
                s2_pos += 1;
            }
            while s1.get(s1_pos) == Some(&b'0') {
                s1_pos += 1;
            }
            while s2.get(s2_pos) == Some(&b'0') {
                s2_pos += 1;
            }

            while s1.digit_at(s1_pos) && s2.digit_at(s2_pos) {
                if first_diff == Ordering::Equal {
                    first_diff = s1.get(s1_pos).cmp(&s2.get(s2_pos));
                }
                s1_pos += 1;
                s2_pos += 1;
            }
            if s1.digit_at(s1_pos) {
                return Ordering::Greater;
            }
            if s2.digit_at(s2_pos) {
                return Ordering::Less;
            }
            if first_diff != Ordering::Equal {
                return first_diff;
            }
        }
        Ordering::Equal
    }
}

trait SliceExt {
    fn digit_at(&self, index: usize) -> bool;
}

impl SliceExt for &[u8] {
    fn digit_at(&self, index: usize) -> bool {
        self.get(index).map(u8::is_ascii_digit).unwrap_or(false)
    }
}

impl<'a> Writable for CStr<'a> {
    fn bytes_repr(&self) -> &[u8] {
        &self.bytes[..self.bytes.len() - 1]
    }
}

impl<'a> PartialEq<&[u8]> for CStr<'a> {
    fn eq(&self, bytes: &&[u8]) -> bool {
        if bytes.last() == Some(&0) {
            self.bytes == *bytes
        } else {
            &self.bytes[..self.bytes.len() - 1] == *bytes
        }
    }
}
