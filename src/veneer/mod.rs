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

// Allocates in chunks of 64 bytes. The `usage_mask` is a bitmask that is 1 where something is
// allocated.
pub struct SmallAllocator {
    slab: [u8; 4096],
    usage_mask: u64,
}

impl SmallAllocator {
    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if layout.align() > 64 || layout.size() == 0 || layout.size() > 4096 {
            return core::ptr::null_mut();
        }

        // Round size up to a multiple of 64
        let remainder = layout.size() % 64;
        let size = if remainder == 0 {
            layout.size()
        } else {
            layout.size() + 64 - remainder
        };
        let blocks = size / 64;
        let my_mask = u64::MAX << (64 - blocks);

        for i in 0..(64 - blocks) {
            if (my_mask >> i) & (!self.usage_mask) == (my_mask >> i) {
                self.usage_mask |= my_mask >> i;
                return self.slab[64 * i..].as_mut_ptr();
            }
        }

        core::ptr::null_mut()
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) -> bool {
        let offset = ptr.offset_from(self.slab.as_mut_ptr());
        if offset < 0 {
            return false;
        }
        let offset = offset as usize;
        if offset >= 4096 {
            return false;
        }

        let offset_blocks = offset / 64;

        let remainder = layout.size() % 64;
        let size = if remainder == 0 {
            layout.size()
        } else {
            layout.size() + 64 - remainder
        };
        let blocks = size / 64;

        for i in 0..blocks {
            self.usage_mask &= !((1 << 63) >> (i + offset_blocks));
        }
        true
    }
}

pub struct Allocator {
    cache: SpinLock<[(bool, *mut u8, usize); 64]>,
    small: SpinLock<SmallAllocator>,
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            cache: SpinLock::new([(false, ptr::null_mut(), 0); 64]),
            small: SpinLock::new(SmallAllocator {
                slab: [0u8; 4096],
                usage_mask: 0u64,
            }),
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
        let small_ptr = self.small.lock().alloc(layout);
        if !small_ptr.is_null() {
            return small_ptr;
        }

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

        // We didn't find a mapping that's already big enough, resize the largest one.
        if let Some((is_used, ptr, len)) = cache
            .iter_mut()
            .filter(|(is_used, ptr, _)| !*is_used && !ptr.is_null())
            .max_by_key(|(_, _, len)| *len)
        {
            *is_used = true;
            *ptr = syscalls::mremap(*ptr, *len, layout.size(), libc::MREMAP_MAYMOVE)
                .unwrap_or(core::ptr::null_mut());
            return *ptr;
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
        if self.small.lock().dealloc(ptr, layout) {
            return;
        }

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
        let mut small = self.small.lock();
        if ptr > small.slab.as_mut_ptr() && ptr < small.slab.as_mut_ptr().add(small.slab.len()) {
            drop(small);
            let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
            let new_ptr = self.alloc(new_layout);
            core::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size());
            self.dealloc(ptr, layout);
            return new_ptr;
        }

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
