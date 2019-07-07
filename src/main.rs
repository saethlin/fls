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

extern crate alloc;
use alloc::vec::Vec;

mod directory;
mod error;
mod output;
mod style;

use directory::Directory;
use error::Error;
use output::*;
use style::Style;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let mut args = Vec::with_capacity(argc as usize);
    unsafe {
        for i in 0..argc {
            let ptr = *argv.offset(i as isize);
            let arg = core::slice::from_raw_parts(ptr, cstr_len(ptr) + 1);
            args.push(arg);
        }
    }

    match run(&args) {
        Ok(()) => 0,
        Err(e) => e.0,
    }
}

fn run(args: &[&[u8]]) -> Result<(), Error> {
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

    let options: Vec<_> = args
        .iter()
        .skip(1)
        .take_while(|a| a.get(0) == Some(&b'-'))
        .collect();

    let mut args: Vec<_> = args
        .iter()
        .skip(1)
        .skip_while(|a| a.get(0) == Some(&b'-'))
        .collect();
    let cwd = &b".\0"[..];
    if args.is_empty() {
        args.push(&cwd);
    }

    if let Ok(width) = terminal_width {
        if options.iter().any(|opt| *opt == b"-l\0") {
            for arg in args.into_iter() {
                write_details(arg, &mut out)?;
            }
        } else {
            for arg in args.into_iter() {
                write_grid(arg, &mut out, width)?;
            }
        }
    } else {
        for arg in args.into_iter() {
            write_single_column(arg, &mut out)?;
        }
    }

    Ok(())
}

unsafe fn cstr_len(cstr: *const u8) -> usize {
    let mut len = 0;
    while *cstr.offset(len as isize) != 0 {
        len += 1
    }
    len
}
