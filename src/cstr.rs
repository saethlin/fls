use crate::Writable;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy)]
pub struct CStr<'a> {
    bytes: &'a [u8],
}

impl<'a> CStr<'a> {
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

    pub fn get(&self, i: usize) -> Option<&u8> {
        self.bytes.get(i)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }

    pub fn len_utf8(&self) -> usize {
        if self.bytes.iter().all(|c| c.is_ascii()) {
            if self.bytes.last() == Some(&0) {
                self.bytes.len() - 1
            } else {
                self.bytes.len()
            }
        } else {
            core::str::from_utf8(self.bytes)
                .map(|s| s.graphemes(false).count())
                .unwrap_or_else(|_| self.bytes.len())
        }
    }
}

impl<'a> Writable for CStr<'a> {
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
