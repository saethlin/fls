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

mod directory;
mod error;
mod output;
mod style;

use directory::DirEntry;
use output::*;
use style::Style;

use veneer::syscalls;
use veneer::CStr;
use veneer::Error;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const libc::c_char) -> i32 {
    let mut args = Vec::with_capacity(argc as usize);
    unsafe {
        for i in 0..argc {
            let ptr = *argv.offset(i as isize);
            args.push(CStr::from_ptr(ptr));
        }
    }

    match run(&args) {
        Ok(()) => 0,
        Err(e) => -1, //e.0 as i32, TODO
    }
}

#[derive(PartialEq, Eq)]
enum DisplayMode {
    Grid(usize),
    Long,
    Column,
}

fn run(args: &[CStr]) -> Result<(), Error> {
    let mut out = BufferedStdout::new();
    let mut uid_usernames = Vec::new();

    let mut display_mode = if let Ok(d) = syscalls::winsize() {
        DisplayMode::Grid(d.ws_col as usize)
    } else {
        DisplayMode::Column
    };

    let mut switches: SmallVec<[u8; 8]> = SmallVec::new();
    for a in args
        .iter()
        .skip(1)
        .filter(|a| a.get(0) == Some(b'-'))
        .take_while(|a| a.as_bytes() != b"--")
    {
        switches.extend_from_slice(&a.as_bytes()[1..]);
    }

    let mut args: Vec<_> = args
        .iter()
        .cloned()
        .skip(1)
        .skip_while(|a| a.get(0) == Some(b'-'))
        .collect();
    if args.is_empty() {
        args.push(CStr::from_bytes(b".\0"));
    }

    let show_all = switches.contains(&b'a');
    if switches.contains(&b'1') {
        display_mode = DisplayMode::Column;
    }
    if switches.contains(&b'l') {
        display_mode = DisplayMode::Long;
    }
    let sort_reversed = switches.contains(&b'r');
    let sort_time = switches.contains(&b't');
    let sort_size = switches.contains(&b'S');
    let need_details = display_mode == DisplayMode::Long || sort_time || sort_size;

    let multiple_args = args.len() > 1;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for arg in args {
        match veneer::Directory::open(arg) {
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

    if !need_details {
        files.sort_unstable_by(|a, b| {
            let ordering = vercmp(a.name(), b.name());
            if sort_reversed {
                ordering.reverse()
            } else {
                ordering
            }
        });

        match display_mode {
            DisplayMode::Grid(width) => write_grid(
                &files,
                &veneer::Directory::open(CStr::from_bytes(b".\0"))?,
                &mut out,
                width,
            )?,
            DisplayMode::Column => write_single_column(&files, &mut out)?,
            DisplayMode::Long => {}
        }
    } else {
        let mut files_and_stats = Vec::with_capacity(files.len());
        let dir = veneer::Directory::open(CStr::from_bytes(b".\0"))?;
        for e in files.iter().cloned() {
            let stats = Status::from(syscalls::lstatat(dir.raw_fd(), e.name())?);
            files_and_stats.push((e, stats));
        }

        files_and_stats.sort_unstable_by(|a, b| {
            let ordering = if sort_time {
                a.1.mtime.cmp(&b.1.mtime)
            } else if sort_size {
                a.1.size.cmp(&b.1.size)
            } else {
                vercmp(a.0.name(), b.0.name())
            };
            if sort_reversed {
                ordering.reverse()
            } else {
                ordering
            }
        });

        match display_mode {
            DisplayMode::Grid(width) => write_grid(&files_and_stats, &dir, &mut out, width)?,
            DisplayMode::Long => write_details(&files_and_stats, &mut uid_usernames, &mut out)?,
            DisplayMode::Column => write_single_column(&files_and_stats, &mut out)?,
        }
    }

    if !dirs.is_empty() && !files.is_empty() {
        out.push(b'\n')?;
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        let contents = dir.read()?;
        let hint = contents.iter().size_hint();
        let mut entries: SmallVec<[veneer::directory::DirEntry; 32]> = SmallVec::new();
        entries.reserve(hint.1.unwrap_or(hint.0));

        if show_all {
            for e in contents.iter() {
                if e.name().as_bytes() != b".." && e.name().as_bytes() != b"." {
                    entries.push(e)
                }
            }
        } else {
            for e in contents.iter().filter(|e| e.name().get(0) != Some(b'.')) {
                entries.push(e)
            }
        };

        if multiple_args {
            out.write(*name)?;
            out.write(b":\n")?;
        }

        if !need_details {
            entries.sort_unstable_by(|a, b| {
                let ordering = vercmp(a.name(), b.name());
                if sort_reversed {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
            match display_mode {
                DisplayMode::Grid(width) => write_grid(&entries, &dir, &mut out, width)?,
                DisplayMode::Column => write_single_column(&entries, &mut out)?,
                DisplayMode::Long => {}
            }
        } else {
            let mut entries_and_stats = Vec::new();
            entries_and_stats.reserve(entries.len());
            for e in entries.iter().cloned() {
                let status = Status::from(syscalls::lstatat(dir.raw_fd(), e.name())?);
                entries_and_stats.push((e, status));
            }
            entries_and_stats.sort_unstable_by(|a, b| {
                let ordering = if sort_time {
                    a.1.mtime.cmp(&b.1.mtime)
                } else if sort_size {
                    a.1.size.cmp(&b.1.size)
                } else {
                    vercmp(a.0.name(), b.0.name())
                };
                if sort_reversed {
                    ordering.reverse()
                } else {
                    ordering
                }
            });

            match display_mode {
                DisplayMode::Grid(width) => write_grid(&entries_and_stats, &dir, &mut out, width)?,
                DisplayMode::Long => {
                    write_details(&entries_and_stats, &mut uid_usernames, &mut out)?
                }
                DisplayMode::Column => write_single_column(&entries_and_stats, &mut out)?,
            }
        }

        if multiple_args && n != dirs.len() - 1 {
            out.push(b'\n')?;
        }
    }

    Ok(())
}

pub struct Status {
    pub mode: u32,
    pub size: i64,
    pub uid: u32,
    pub mtime: i64,
}

impl Status {
    fn style(&self) -> Option<Style> {
        let entry_type = self.mode & libc::S_IFMT;
        if entry_type == libc::S_IFDIR {
            Some(Style::BlueBold)
        } else if entry_type == libc::S_IFLNK {
            Some(Style::Cyan)
        } else if self.mode & libc::S_IXUSR > 0 {
            Some(Style::GreenBold)
        } else {
            None
        }
    }
}

impl From<libc::stat64> for Status {
    fn from(stats: libc::stat64) -> Self {
        Self {
            mode: stats.st_mode,
            size: stats.st_size,
            uid: stats.st_uid,
            mtime: stats.st_mtime,
        }
    }
}
