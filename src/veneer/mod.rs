pub mod cstr;
pub mod directory;
pub mod error;
mod spinlock;
pub mod syscalls;

pub use cstr::*;
pub use directory::*;
pub use error::*;
use spinlock::SpinLock;

use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

pub struct Allocator {
    cache: SpinLock<[(bool, *mut u8, usize); 64]>,
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            cache: SpinLock::new([(false, ptr::null_mut(), 0); 64]),
        }
    }
}

fn round_to_page(layout: Layout) -> Layout {
    let remainder = layout.size() % 4096;
    let size = if remainder == 0 {
        layout.size()
    } else {
        layout.size() + 4096 - remainder
    };
    Layout::from_size_align(size, layout.align()).unwrap()
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, mut layout: Layout) -> *mut u8 {
        layout = round_to_page(layout);

        let mut cache = self.cache.lock();

        // Find the closest fit
        if let Some((is_used, ptr, _)) = cache
            .iter_mut()
            .filter(|(is_used, _, len)| !*is_used && *len >= layout.size())
            .min_by_key(|(_, _, len)| *len - layout.size())
        {
            *is_used = true;
            return *ptr;
        }

        // We didn't find a mapping that's already big enough, look for one that we can resize
        for (is_used, ptr, len) in cache.iter_mut() {
            if !*is_used && !ptr.is_null() {
                *is_used = true;
                *ptr = syscalls::mremap(*ptr, *len, layout.size(), libc::MREMAP_MAYMOVE)
                    .unwrap_or(core::ptr::null_mut());
                return *ptr;
            }
        }

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

    unsafe fn dealloc(&self, ptr: *mut u8, mut layout: Layout) {
        layout = round_to_page(layout);

        let mut cache = self.cache.lock();

        // Look for this entry in the cache and mark it as unused
        for (is_used, cache_ptr, _len) in cache.iter_mut() {
            if ptr == *cache_ptr {
                *is_used = false;
                return;
            }
        }

        // We didn't find it in the cache, try to add it
        for (is_used, cache_ptr, len) in cache.iter_mut() {
            if !*is_used {
                *cache_ptr = ptr;
                *len = layout.size();
                return;
            }
        }
        syscalls::munmap(ptr, layout.size()).unwrap();
    }

    unsafe fn realloc(&self, ptr: *mut u8, mut layout: Layout, mut new_size: usize) -> *mut u8 {
        layout = round_to_page(layout);
        let remainder = new_size % 4096;
        new_size = if remainder == 0 {
            new_size
        } else {
            new_size + 4096 - remainder
        };

        if layout.size() >= new_size {
            return ptr;
        }

        let mut cache = self.cache.lock();

        for (is_used, cache_ptr, len) in cache.iter_mut() {
            if *cache_ptr == ptr {
                *len = new_size;
                assert!(*is_used);
                *cache_ptr = syscalls::mremap(ptr, layout.size(), new_size, libc::MREMAP_MAYMOVE)
                    .unwrap_or(core::ptr::null_mut());
                return *cache_ptr;
            }
        }

        syscalls::mremap(ptr, layout.size(), new_size, libc::MREMAP_MAYMOVE)
            .unwrap_or(core::ptr::null_mut())
    }
}
