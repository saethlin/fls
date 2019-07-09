#![no_std]
#![no_main]
#![feature(lang_items, alloc_error_handler)]
#![feature(ptr_offset_from)]

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
use smallvec::SmallVec;

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

#[derive(Clone, Copy)]
pub struct CStr<'a> {
    bytes: &'a [u8],
}

impl<'a> CStr<'a> {
    pub unsafe fn from_ptr(ptr: *const u8) -> CStr<'a> {
        CStr {
            bytes: core::slice::from_raw_parts(ptr, libc::strlen(ptr as *const i8) + 1),
        }
    }

    pub fn get(&self, i: usize) -> Option<&u8> {
        self.bytes.get(i)
    }

    fn iter(&self) -> impl Iterator<Item = &u8> {
        self.bytes.iter()
    }

    fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

impl<'a> AsRef<[u8]> for CStr<'a> {
    fn as_ref(&self) -> &[u8] {
        self.bytes
    }
}

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

    let terminal_width = terminal_size().map(|d| d.ws_col as usize);

    let mut options: SmallVec<[u8; 8]> = SmallVec::new();
    for a in args.iter().skip(1).take_while(|a| a.get(0) == Some(&b'-')) {
        options.extend(a.iter().cloned().take_while(|a| *a != 0));
    }

    let mut args: Vec<CStr> = args
        .iter()
        .cloned()
        .skip(1)
        .skip_while(|a| a.get(0) == Some(&b'-'))
        .collect();
    let cwd = CStr { bytes: &b".\0"[..] };
    if args.is_empty() {
        args.push(cwd);
    }

    let show_all = options.contains(&b'a');

    if let Ok(width) = terminal_width {
        if options.contains(&b'l') {
            for arg in args.into_iter() {
                write_details(arg, &mut out, show_all)?;
            }
        } else {
            for arg in args.into_iter() {
                write_grid(arg, &mut out, width, show_all)?;
            }
        }
    } else {
        for arg in args.into_iter() {
            write_single_column(arg, &mut out, show_all)?;
        }
    }

    Ok(())
}

fn terminal_size() -> Result<libc::winsize, Error> {
    unsafe {
        let mut winsize: libc::winsize = core::mem::zeroed();
        let ret = syscall::syscall!(
            IOCTL,
            libc::STDOUT_FILENO,
            libc::TIOCGWINSZ,
            &mut winsize as *mut libc::winsize
        ) as isize;
        if ret < 0 {
            Err(-ret)?
        } else {
            Ok(winsize)
        }
    }
}
