use core::ffi::c_int;

#[derive(Clone, Copy)]
pub struct Error(pub c_int);

impl PartialEq<i32> for Error {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 == *other
    }
}

impl core::fmt::Debug for Error {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        <Self as core::fmt::Display>::fmt(self, f)
    }
}

impl core::fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "OS Error {}", self.0)
    }
}
