pub mod cstr;
pub mod directory;
pub mod error;
pub mod syscalls;

pub use cstr::*;
pub use directory::*;
pub use error::*;

use core::alloc::{GlobalAlloc, Layout};

pub struct Allocator;

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_zeroed(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        syscalls::munmap(ptr, layout.size()).unwrap();
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        syscalls::mremap(ptr, layout.size(), new_size, libc::MREMAP_MAYMOVE)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        syscalls::mmap(
            core::ptr::null_mut(),
            layout.size(),
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANON | libc::MAP_PRIVATE,
            -1,
            0,
        )
        .unwrap_or(core::ptr::null_mut())
    }
}
