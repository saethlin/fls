#![no_std]
#![no_main]
#![feature(lang_items, alloc_error_handler)]

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern "C" fn rust_eh_unwind_resume() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    loop {}
}

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

extern crate alloc;
use arrayvec::ArrayVec;

mod directory;
mod error;
mod output;
use output::*;

use error::Error;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    match run(argc, argv) {
        Ok(()) => 0,
        Err(e) => e.0,
    }
}

fn run(argc: i32, argv: *const *const u8) -> Result<(), Error> {
    let mut out = BufferedStdout::new();

    let terminal_width = unsafe {
        let mut winsize = [0u16; 4];
        let ret = syscall::syscall!(IOCTL, 1, 0x5413, winsize.as_mut_ptr()) as i32;
        if ret < 0 {
            Err(-ret)
        } else {
            Ok(winsize[1] as usize)
        }
    };

    let mut root: ArrayVec<[u8; 4096 as usize]> = ArrayVec::new();
    if argc < 2 {
        root.try_push(b'.')?;
        root.try_push(0)?;
        if let Ok(width) = terminal_width {
            write_grid(&root, &mut out, width)?;
        } else {
            write_single_column(&root, &mut out)?;
        }
    } else {
        for a in 1..argc {
            root.clear();
            unsafe {
                let mut arg: *const u8 = *argv.offset(a as isize);
                loop {
                    let b = *arg;
                    if b == 0 {
                        break;
                    }
                    root.try_push(b)?;
                    arg = arg.offset(1);
                }
            }
            out.write_all(&root)?;
            out.write_all(b":\n")?;
            root.try_push(0)?;
            if let Ok(width) = terminal_width {
                write_grid(&root, &mut out, width)?;
            } else {
                write_single_column(&root, &mut out)?;
            }
            if a != argc - 1 {
                out.write_all(b"\n")?;
            }
        }
    }

    Ok(())
}
