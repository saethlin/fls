#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, ch: i32, count: usize) {
    let ch = ch as u8;
    for i in 0..count {
        *dest.add(i) = ch;
    }
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, count: usize) {
    let end = src.add(count).cast::<u32>();
    let mut dst = dst.cast::<u32>();
    let mut src = src.cast::<u32>();

    while end > src.add(1) {
        dst.write_unaligned(src.read_unaligned());
        dst = dst.add(1);
        src = src.add(1);
    }

    let end = end.cast::<u8>();
    let mut dst = dst.cast::<u8>();
    let mut src = src.cast::<u8>();
    while end > src {
        *dst = *src;
        dst = dst.add(1);
        src = src.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn memmove(mut dst: *mut u8, mut src: *const u8, count: usize) {
    let end = src.add(count);
    while end > src {
        *dst = *src;
        dst = dst.add(1);
        src = src.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(lhs: *mut u8, rhs: *const u8, count: usize) -> i32 {
    for i in 0..count {
        let l = *lhs.add(i);
        let r = *rhs.add(i);
        if l > r {
            return 1;
        }
        if r > l {
            return -1;
        }
    }
    0
}
