use crate::CStr;
use core::sync::atomic::{AtomicIsize, AtomicPtr, Ordering::SeqCst};

pub(crate) static ARGC: AtomicIsize = AtomicIsize::new(-1);
pub(crate) static ARGV: AtomicPtr<*const u8> = AtomicPtr::new(core::ptr::null_mut());

#[inline]
pub fn args() -> impl Iterator<Item = CStr<'static>> {
    unsafe {
        let argc = ARGC.load(SeqCst);
        let argv = ARGV.load(SeqCst);
        assert!(!argv.is_null() && argc != -1);
        (0..argc).map(move |i| CStr::from_ptr(*argv.offset(i)))
    }
}
