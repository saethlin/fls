use crate::Writable;

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
        assert!(bytes.last() == Some(&0));
        CStr { bytes }
    }

    pub fn get(&self, i: usize) -> Option<&u8> {
        self.bytes.get(i)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

impl<'a> Writable for CStr<'a> {
    fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
