#![no_main]
#![no_std]
#![feature(lang_items, alloc_error_handler)]

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern "C" fn rust_eh_unwind_resume() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { libc::abort() }
}

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    unsafe { libc::abort() }
}

struct Allocator;

use core::alloc::{GlobalAlloc, Layout};
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        libc::malloc(layout.size()) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        libc::free(ptr as *mut libc::c_void)
    }
    unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
        libc::realloc(ptr as *mut libc::c_void, new_size) as *mut u8
    }
}

#[global_allocator]
static ALLOC: Allocator = Allocator;

extern crate alloc;
use alloc::vec::Vec;
use smallvec::SmallVec;

mod cstr;
mod directory;
mod error;
mod output;
mod style;
mod syscalls;

use cstr::CStr;
use directory::{DirEntry, Directory, RawDirEntry};
use error::Error;
use output::*;
use style::Style;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    let mut args = Vec::with_capacity(argc as usize);
    unsafe {
        for i in 0..argc {
            let ptr = *argv.offset(i as isize);
            args.push(CStr::from_ptr(ptr));
        }
    }

    match run(&args) {
        Ok(()) => 0,
        Err(e) => e.0 as i32,
    }
}

fn run(args: &[CStr]) -> Result<(), Error> {
    let mut out = BufferedStdout::new();

    let terminal_width = syscalls::winsize().map(|d| d.ws_col as usize);

    let mut switches: SmallVec<[u8; 8]> = SmallVec::new();
    for a in args
        .iter()
        .skip(1)
        .take_while(|a| a.get(0) == Some(&b'-') && a.get(1) != Some(&b'0'))
    {
        switches.extend_from_slice(&a.as_bytes()[1..]);
    }

    let mut args: Vec<_> = args
        .iter()
        .cloned()
        .skip(1)
        .skip_while(|a| a.get(0) == Some(&b'-'))
        .collect();
    if args.is_empty() {
        args.push(CStr::from_bytes(b".\0"));
    }

    // TODO: Do we even want to do this?
    args.sort_by(|a, b| a.vercmp(*b));

    let show_all = switches.contains(&b'a');
    let multiple_args = args.len() > 1;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for arg in args {
        match Directory::open(arg) {
            Ok(d) => dirs.push((arg, d)),
            Err(Error(20)) => files.push(crate::directory::File { path: arg }),
            Err(Error(2)) => {
                out.write(arg)?;
                out.write(b": No such file or directory\n")?;
            }
            Err(Error(13)) => {
                out.write(arg)?;
                out.write(b": Permission denied\n")?;
            }
            Err(e) => {
                out.write(arg)?;
                out.write(b": OS error ")?;
                let mut buf = itoa::Buffer::new();
                out.write(buf.format(e.0).as_bytes())?;
                out.push(b'\n')?;
            }
        }
    }

    if let Ok(width) = terminal_width {
        if switches.contains(&b'l') {
            write_details(
                &Directory::open(CStr::from_bytes(b".\0"))?,
                &files,
                &mut out,
            )?;
        } else {
            write_grid(&files, &mut out, width)?;
        }
    } else {
        write_single_column(&files, &mut out)?;
    }

    if !dirs.is_empty() && !files.is_empty() {
        out.push(b'\n')?;
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        let hint = dir.iter().size_hint();
        let mut entries: SmallVec<[RawDirEntry; 32]> = SmallVec::new();
        entries.reserve(hint.1.unwrap_or(hint.0));

        if show_all {
            for e in dir.iter() {
                if e.name() != b".." && e.name() != b"." {
                    entries.push(e)
                }
            }
        } else {
            for e in dir.iter().filter(|e| e.name().get(0) != Some(&b'.')) {
                entries.push(e)
            }
        }

        if switches.contains(&b'r') {
            entries.sort_by(|a, b| b.name().vercmp(a.name()));
        } else {
            entries.sort_by(|a, b| a.name().vercmp(b.name()));
        }

        if multiple_args {
            out.write(*name)?;
            out.write(b":\n")?;
        }

        if let Ok(width) = terminal_width {
            if switches.contains(&b'l') {
                write_details(dir, &entries, &mut out)?;
            } else {
                write_grid(&entries, &mut out, width)?;
            }
        } else {
            write_single_column(&entries, &mut out)?;
        }

        if multiple_args && n != dirs.len() - 1 {
            out.push(b'\n')?;
        }
    }

    Ok(())
}
