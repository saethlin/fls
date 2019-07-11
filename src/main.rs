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

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

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

    let mut options: SmallVec<[u8; 8]> = SmallVec::new();
    for a in args.iter().skip(1).take_while(|a| a.get(0) == Some(&b'-')) {
        options.extend_from_slice(a.as_bytes());
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

    args.sort_by(|a, b| vercmp(a.as_bytes(), b.as_bytes()));

    let show_all = options.contains(&b'a');
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
        if options.contains(&b'l') {
            write_details(CStr::from_bytes(b".\0"), &files, &mut out)?;
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
                entries.push(e)
            }
        } else {
            for e in dir.iter().filter(|e| e.name().get(0) != Some(&b'.')) {
                entries.push(e)
            }
        }

        entries.sort_by(|a, b| vercmp(a.name(), b.name()));

        if multiple_args {
            out.write(*name)?;
            out.write(b":\n")?;
        }

        if let Ok(width) = terminal_width {
            if options.contains(&b'l') {
                write_details(*name, &entries, &mut out)?;
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

use core::cmp::Ordering;
fn vercmp(s1: &[u8], s2: &[u8]) -> Ordering {
    let mut s1_pos: usize = 0;
    let mut s2_pos: usize = 0;

    while s1_pos < s1.len() || s2_pos < s2.len() {
        let mut first_diff = Ordering::Equal;
        while (s1_pos < s1.len() && !s1.digit_at(s1_pos))
            || (s2_pos < s2.len() && !s2.digit_at(s2_pos))
        {
            let s1_c = s1.get(s1_pos).map(u8::to_ascii_lowercase);
            let s2_c = s2.get(s2_pos).map(u8::to_ascii_lowercase);
            if s1_c != s2_c {
                return s1_c.cmp(&s2_c);
            }
            s1_pos += 1;
            s2_pos += 1;
        }
        while s1.get(s1_pos) == Some(&b'0') {
            s1_pos += 1;
        }
        while s2.get(s2_pos) == Some(&b'0') {
            s2_pos += 1;
        }

        while s1.digit_at(s1_pos) && s2.digit_at(s2_pos) {
            if first_diff == Ordering::Equal {
                first_diff = s1.get(s1_pos).cmp(&s2.get(s2_pos));
            }
            s1_pos += 1;
            s2_pos += 1;
        }
        if s1.digit_at(s1_pos) {
            return Ordering::Greater;
        }
        if s2.digit_at(s2_pos) {
            return Ordering::Less;
        }
        if first_diff != Ordering::Equal {
            return first_diff;
        }
    }
    Ordering::Equal
}

trait SliceExt {
    fn digit_at(&self, index: usize) -> bool;
}

impl SliceExt for &[u8] {
    fn digit_at(&self, index: usize) -> bool {
        self.get(index).map(u8::is_ascii_digit).unwrap_or(false)
    }
}
