use core::{fmt, str};

#[derive(Clone, Copy, PartialEq)]
pub struct CStr<'a> {
    bytes: &'a [u8],
}

impl Default for CStr<'static> {
    fn default() -> Self {
        CStr::from_bytes(&[0])
    }
}

impl<'a> CStr<'a> {
    /// # Safety
    ///
    /// This function must be called with a pointer to a null-terminated array of bytes
    #[inline]
    pub unsafe fn from_ptr<'b>(ptr: *const u8) -> CStr<'b> {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        CStr {
            bytes: core::slice::from_raw_parts(ptr.cast::<u8>(), len + 1),
        }
    }

    #[inline]
    pub fn from_bytes(bytes: &'a [u8]) -> CStr<'a> {
        assert!(
            bytes.last() == Some(&0),
            "attempted to construct a CStr from a slice without a null terminator"
        );
        CStr { bytes }
    }

    #[inline]
    pub fn as_bytes(&self) -> &'a [u8] {
        unsafe { self.bytes.get_unchecked(..self.bytes.len() - 1) }
    }

    #[inline]
    pub fn get(&self, i: usize) -> Option<u8> {
        self.bytes.get(i).copied()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

impl<'a> core::ops::Deref for CStr<'a> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl<'a> PartialEq<&[u8]> for CStr<'a> {
    #[inline]
    fn eq(&self, bytes: &&[u8]) -> bool {
        if bytes.last() == Some(&0) {
            self.bytes == *bytes
        } else {
            &self.bytes[..self.bytes.len() - 1] == *bytes
        }
    }
}

impl<'a> PartialEq<&str> for CStr<'a> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        &self.bytes[..self.bytes.len() - 1] == other.as_bytes()
    }
}

impl<'a> fmt::Debug for CStr<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match str::from_utf8(self.as_bytes()) {
            Ok(s) => s.fmt(f),
            Err(e) => str::from_utf8(&self.as_bytes()[..e.valid_up_to()])
                .unwrap()
                .fmt(f),
        }
    }
}

impl<'a> fmt::Display for CStr<'a> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match str::from_utf8(self.as_bytes()) {
            Ok(s) => s.fmt(f),
            Err(e) => str::from_utf8(&self.as_bytes()[..e.valid_up_to()])
                .unwrap()
                .fmt(f),
        }
    }
}
