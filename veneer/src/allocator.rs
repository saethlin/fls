use crate::{libc, spinlock::SpinLock, syscalls};
use core::{
    alloc::{GlobalAlloc, Layout},
    mem::{self, MaybeUninit},
    ptr,
};

const PAGE_SIZE: usize = mem::size_of::<Page>();
const PAGE_ALIGN: usize = mem::align_of::<Page>();
const TOTAL_PAGES: usize = 64;

#[derive(Clone, Copy)]
#[repr(align(4096))]
pub(crate) struct Page(#[allow(dead_code)] pub(crate) [MaybeUninit<u8>; 4096]);

// Allocates in chunks of 64 bytes. The `usage_mask` is a bitmask that is 1 where something is
// allocated.
#[repr(C)]
struct SmallAllocator {
    pages: *mut [Page; TOTAL_PAGES],
    usage_mask: u64,
}

impl SmallAllocator {
    fn to_pages(size: usize) -> usize {
        let remainder = size % PAGE_SIZE;
        let size = if remainder == 0 {
            size
        } else {
            size + PAGE_SIZE - remainder
        };
        size / PAGE_SIZE
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        assert!(!self.pages.is_null());
        if layout.align() > PAGE_ALIGN
            || layout.size() == 0
            || layout.size() > PAGE_SIZE * TOTAL_PAGES
        {
            return core::ptr::null_mut();
        }

        let pages = Self::to_pages(layout.size());

        let my_mask = u64::MAX << (TOTAL_PAGES - pages);

        for i in 0..=(TOTAL_PAGES - pages) {
            if ((my_mask >> i) & self.usage_mask) == 0 {
                self.usage_mask |= my_mask >> i;
                unsafe {
                    return self.pages.cast::<u8>().add(PAGE_SIZE * i);
                }
            }
        }

        core::ptr::null_mut()
    }

    fn dealloc(&mut self, ptr: *mut u8, layout: Layout) -> bool {
        let offset = ptr.addr().wrapping_sub(self.pages.addr());
        if !(0..PAGE_SIZE * TOTAL_PAGES).contains(&offset) {
            return false;
        }

        let offset_pages = offset / PAGE_SIZE;
        let pages = Self::to_pages(layout.size());

        let my_mask = (u64::MAX << (TOTAL_PAGES - pages)) >> offset_pages;

        assert!(my_mask & self.usage_mask == my_mask);
        self.usage_mask ^= my_mask;

        true
    }
}

pub(crate) struct Allocator {
    imp: SpinLock<AllocatorImpl>,
}

pub(crate) struct AllocatorImpl {
    small: SmallAllocator,
    cache: [(bool, *mut u8, usize); 64],
}

impl Allocator {
    pub(crate) const fn new() -> Self {
        Self {
            imp: SpinLock::new(AllocatorImpl {
                cache: [(false, ptr::null_mut(), 0); 64],
                small: SmallAllocator {
                    pages: ptr::null_mut(),
                    usage_mask: 0,
                },
            }),
        }
    }

    pub(crate) fn init(&self, pages: *mut [Page; 64]) {
        self.imp.lock().small.pages = pages;
    }
}

fn round_to_page(layout: Layout) -> Layout {
    let remainder = layout.size() % PAGE_SIZE;
    let size = if remainder == 0 {
        layout.size()
    } else {
        layout.size() + PAGE_SIZE - remainder
    };
    match Layout::from_size_align(size, layout.align()) {
        Ok(l) => l,
        Err(_) => alloc::alloc::handle_alloc_error(layout),
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.imp.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.imp.lock().dealloc(ptr, layout)
    }
}

impl AllocatorImpl {
    fn alloc(&mut self, mut layout: Layout) -> *mut u8 {
        let small_ptr = self.small.alloc(layout);
        if !small_ptr.is_null() {
            return small_ptr;
        }

        layout = round_to_page(layout);

        // Find the closest fit
        if let Some((is_used, ptr, _)) = self
            .cache
            .iter_mut()
            .filter(|(is_used, _, len)| !*is_used && *len >= layout.size())
            .min_by_key(|(_, _, len)| *len - layout.size())
        {
            *is_used = true;
            return *ptr;
        }

        // We didn't find a mapping that's already big enough, resize the largest one.
        if let Some((is_used, ptr, len)) = self
            .cache
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

    unsafe fn dealloc(&mut self, ptr: *mut u8, mut layout: Layout) {
        if self.small.dealloc(ptr, layout) {
            return;
        }

        layout = round_to_page(layout);
        // Look for this entry in the cache and mark it as unused
        for (is_used, cache_ptr, _len) in self.cache.iter_mut() {
            if ptr == *cache_ptr {
                *is_used = false;
                return;
            }
        }

        // We didn't find it in the cache, try to add it
        for (is_used, cache_ptr, len) in self.cache.iter_mut() {
            if !*is_used {
                *cache_ptr = ptr;
                *len = layout.size();
                return;
            }
        }
        // This is technically fallible, but it seems like there isn't a way to indicate failure
        // when deallocating.
        let _ = syscalls::munmap(ptr, layout.size());
    }

    /*
    unsafe fn realloc(&self, ptr: *mut u8, mut layout: Layout, mut new_size: usize) -> *mut u8 {
        let new_ptr = self.small.lock(|small| {
            let offset = ptr.offset_from(addr_of!((*small).slab[0]));
            if (0..4096).contains(&offset) {
                let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
                let new_ptr = self.alloc(new_layout);
                core::ptr::copy_nonoverlapping(ptr, new_ptr, core::cmp::min(layout.size(), new_size));
                self.dealloc(ptr, layout);
                return new_ptr;
            }
            ptr::null_mut()
        });
        if !new_ptr.is_null() {
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

        let cached_ptr = self.cache.lock(|cache| {
            for (is_used, cache_ptr, len) in (*cache).iter_mut() {
                if *cache_ptr == ptr {
                    *len = new_size;
                    assert!(*is_used);
                    *cache_ptr = syscalls::mremap(ptr, layout.size(), new_size, libc::MREMAP_MAYMOVE)
                        .unwrap_or(core::ptr::null_mut());
                    return *cache_ptr;
                }
            }
            ptr::null_mut()
        });
        if !cached_ptr.is_null() {
            return cached_ptr;
        }

        syscalls::mremap(ptr, layout.size(), new_size, libc::MREMAP_MAYMOVE)
            .unwrap_or(core::ptr::null_mut())
    }
    */
}

impl Drop for Allocator {
    fn drop(&mut self) {
        for (_, ptr, len) in self.imp.lock().cache {
            unsafe {
                let _ = syscalls::munmap(ptr, len);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small() {
        let mut pages = [Page([MaybeUninit::uninit(); 4096]); 64];
        let mut alloc = SmallAllocator {
            pages: &mut pages,
            usage_mask: 0,
        };

        for size in 1..4096 {
            println!("{}", size);
            let remainder = size % PAGE_SIZE;
            let rounded = if remainder == 0 {
                size
            } else {
                size + PAGE_SIZE - remainder
            };
            let blocks = rounded / PAGE_SIZE;

            assert!(blocks * PAGE_SIZE >= size);

            let layout = Layout::from_size_align(size, 1).unwrap();

            let ptr = alloc.alloc(layout);

            assert!(!ptr.is_null());

            assert_eq!(blocks, alloc.usage_mask.count_ones() as usize);
            assert!(alloc.dealloc(ptr, layout));
            assert_eq!(0, alloc.usage_mask.count_ones());
        }
    }
}
