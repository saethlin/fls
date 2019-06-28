use core::alloc::{GlobalAlloc, Layout};
use core::mem::size_of;
use core::ptr;
use syscall::syscall;

static mut most_recent_allocation: *mut Block = ptr::null_mut();
static mut remaining_bytes: isize = 0;

pub struct MyAllocator;

struct Block {
    previous_block: *mut Block,
    len: isize,
}

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let bytes_required = ((layout.size() + layout.size() % 8) + size_of::<Block>()) as isize;

        if bytes_required > remaining_bytes {
            // Expand our memory space with another page
            let ret = syscall!(MMAP, 0, 4096, 3, 0x0020 | 0x0002, -1isize, 0) as isize;
            if ret < 0 {
                return ptr::null_mut();
            }
            let new_allocation = ret as *mut Block;
            remaining_bytes = 4096 - size_of::<Block>() as isize;
            most_recent_allocation = new_allocation;
            (*new_allocation).previous_block = ptr::null_mut();
            (*new_allocation).len = bytes_required;
            return new_allocation.offset(1) as *mut u8;
        }

        // Do the allocation
        // Write in the block at the beginning of the new allocation
        remaining_bytes -= bytes_required;
        let previous_block = most_recent_allocation as *mut Block;
        let new_block = (previous_block as *mut u8).offset((*previous_block).len) as *mut Block;
        most_recent_allocation = new_block;
        (*new_block).previous_block = previous_block;
        (*new_block).len = bytes_required;

        (new_block as *mut u8).offset(16)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr == most_recent_allocation.offset(1) as *mut u8 {
            remaining_bytes += (*most_recent_allocation).len;
            most_recent_allocation = (*most_recent_allocation).previous_block;
        }
    }
}
