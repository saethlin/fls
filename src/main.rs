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

#[global_allocator]
static ALLOC: veneer::LibcAllocator = veneer::LibcAllocator;

extern crate alloc;
use alloc::vec::Vec;
use smallvec::SmallVec;

pub mod cli;
mod directory;
mod error;
mod output;
mod style;

use cli::{DisplayMode, ShowAll, SortField};
use directory::DirEntry;
use output::*;
use style::Style;

use veneer::syscalls;
use veneer::CStr;
use veneer::Error;

#[no_mangle]
pub extern "C" fn main(argc: i32, argv: *const *const libc::c_char) -> i32 {
    let mut args = Vec::with_capacity(argc as usize);
    for i in 0..argc {
        args.push(unsafe { CStr::from_ptr(*argv.offset(i as isize)) });
    }

    match run(&args) {
        Ok(()) => 0,
        Err(e) => e.0 as i32,
    }
}

fn run(args: &[CStr<'static>]) -> Result<(), Error> {
    let (mut opt, args) = cli::parse_arguments(args)?;

    let mut uid_usernames = Vec::new();

    let terminal_width = syscalls::winsize().ok().map(|d| d.ws_col as usize);

    match (terminal_width, opt.display_mode) {
        (Some(width), DisplayMode::Grid(_)) => opt.display_mode = DisplayMode::Grid(width),
        (None, DisplayMode::Grid(_)) => opt.display_mode = DisplayMode::SingleColumn,
        _ => {}
    }
    let mut out = if terminal_width.is_some() {
        BufferedStdout::terminal()
    } else {
        BufferedStdout::file()
    };

    let need_details = opt.display_mode == DisplayMode::Long
        || opt.sort_field == Some(SortField::Time)
        || opt.sort_field == Some(SortField::Size);

    let multiple_args = args.len() > 1;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for arg in args {
        match veneer::Directory::open(arg) {
            Ok(d) => dirs.push((arg, d)),
            Err(Error(20)) => files.push(crate::directory::File { path: arg }),
            Err(e) => {
                let mut buf = itoa::Buffer::new();
                out.write(arg)
                    .write(b": OS error ")
                    .write(buf.format(e.0).as_bytes())
                    .push(b' ')
                    .write(e.msg().as_bytes())
                    .push(b'\n');
            }
        }
    }

    if !files.is_empty() {
        if !need_details {
            files.sort_unstable_by(|a, b| {
                let mut ordering = vercmp(a.name(), b.name());
                if opt.reverse_sorting {
                    ordering = ordering.reverse();
                }
                ordering
            });

            match opt.display_mode {
                DisplayMode::Grid(width) => write_grid(
                    &files,
                    &veneer::Directory::open(CStr::from_bytes(b".\0"))?,
                    &mut out,
                    width,
                ),
                DisplayMode::SingleColumn => write_single_column(&files, &mut out),
                DisplayMode::Long | DisplayMode::Stream => {}
            }
        } else {
            let mut files_and_stats = Vec::with_capacity(files.len());
            let dir = veneer::Directory::open(CStr::from_bytes(b".\0"))?;
            for e in files.iter().cloned() {
                let stats = Status::from(syscalls::lstatat(dir.raw_fd(), e.name())?);
                files_and_stats.push((e, stats));
            }

            if let Some(field) = opt.sort_field {
                files_and_stats.sort_unstable_by(|a, b| {
                    let mut ordering = match field {
                        SortField::Time => a.1.mtime.cmp(&b.1.mtime),
                        SortField::Size => a.1.size.cmp(&b.1.size),
                        SortField::Name => vercmp(a.0.name(), b.0.name()),
                    };
                    if opt.reverse_sorting {
                        ordering = ordering.reverse();
                    }
                    ordering
                });
            }

            match opt.display_mode {
                DisplayMode::Grid(width) => write_grid(&files_and_stats, &dir, &mut out, width),
                DisplayMode::Long | DisplayMode::Stream => {
                    write_details(&files_and_stats, &mut uid_usernames, &mut out)
                }
                DisplayMode::SingleColumn => write_single_column(&files_and_stats, &mut out),
            }
        }
    }

    if !dirs.is_empty() && !files.is_empty() {
        out.push(b'\n');
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        let contents = dir.read()?;
        let hint = contents.iter().size_hint();
        let mut entries: SmallVec<[veneer::directory::DirEntry; 32]> = SmallVec::new();
        entries.reserve(hint.1.unwrap_or(hint.0));

        match opt.show_all {
            ShowAll::No => {
                for e in contents.iter().filter(|e| e.name().get(0) != Some(b'.')) {
                    entries.push(e)
                }
            }
            ShowAll::Almost => {
                for e in contents.iter() {
                    if e.name().as_bytes() != b".." && e.name().as_bytes() != b"." {
                        entries.push(e)
                    }
                }
            }
            ShowAll::Yes => {
                for e in contents.iter() {
                    entries.push(e);
                }
            }
        }

        if multiple_args {
            out.write(*name).write(b":\n");
        }

        if !need_details {
            entries.sort_unstable_by(|a, b| {
                let mut ordering = vercmp(a.name(), b.name());
                if opt.reverse_sorting {
                    ordering = ordering.reverse();
                }
                ordering
            });
            match opt.display_mode {
                DisplayMode::Grid(width) => write_grid(&entries, &dir, &mut out, width),
                DisplayMode::SingleColumn => write_single_column(&entries, &mut out),
                DisplayMode::Long | DisplayMode::Stream => {}
            }
        } else {
            let mut entries_and_stats = Vec::new();
            entries_and_stats.reserve(entries.len());
            for e in entries.iter().cloned() {
                let status = Status::from(syscalls::lstatat(dir.raw_fd(), e.name())?);
                entries_and_stats.push((e, status));
            }

            if let Some(field) = opt.sort_field {
                entries_and_stats.sort_unstable_by(|a, b| {
                    let mut ordering = match field {
                        SortField::Time => a.1.mtime.cmp(&b.1.mtime),
                        SortField::Size => a.1.size.cmp(&b.1.size),
                        SortField::Name => vercmp(a.0.name(), b.0.name()),
                    };
                    if opt.reverse_sorting {
                        ordering = ordering.reverse();
                    }
                    ordering
                });
            }

            match opt.display_mode {
                DisplayMode::Grid(width) => write_grid(&entries_and_stats, &dir, &mut out, width),
                DisplayMode::Long | DisplayMode::Stream => {
                    write_details(&entries_and_stats, &mut uid_usernames, &mut out)
                }
                DisplayMode::SingleColumn => write_single_column(&entries_and_stats, &mut out),
            }
        }

        if multiple_args && n != dirs.len() - 1 {
            out.push(b'\n');
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
