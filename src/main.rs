#![no_main]
#![no_std]
#![feature(lang_items, alloc_error_handler)]
// Functions should not be broken up unless they contain reusable parts
#![allow(clippy::cognitive_complexity)]

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

mod output;

macro_rules! error {
    ($($item:expr),+) => {
        {
        use crate::output::Writable;
        use alloc::vec::Vec;
        let mut output = Vec::new();
        output.extend(b"fls: ");
        $(output.extend_from_slice($item.bytes_repr());)*
        let _ = veneer::syscalls::write(2, output.as_slice());
    }};
}

extern crate alloc;
use alloc::vec::Vec;
use smallvec::SmallVec;

pub mod cli;
mod directory;
mod error;
mod style;

use cli::{DisplayMode, ShowAll, SortField};
use directory::DirEntry;
use output::*;
use style::Style;

use veneer::syscalls;
use veneer::CStr;
use veneer::Error;

#[no_mangle]
pub unsafe extern "C" fn main(argc: isize, argv: *const *const libc::c_char) -> i32 {
    let args = (0..argc).map(|i| CStr::from_ptr(*argv.offset(i))).collect();

    match run(args) {
        Ok(()) => 0,
        Err(e) => e.0 as i32,
    }
}

fn run(args: Vec<CStr<'static>>) -> Result<(), Error> {
    let mut app = cli::App::from_arguments(args)?;

    let need_details = app.display_mode == DisplayMode::Long
        || app.sort_field == Some(SortField::Time)
        || app.sort_field == Some(SortField::Size);

    let multiple_args = app.args.len() > 1;

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    for arg in app.args.clone() {
        match veneer::Directory::open(arg) {
            Ok(d) => dirs.push((arg, d)),
            Err(Error(20)) => files.push(crate::directory::File { path: arg }),
            Err(e) => {
                let mut buf = itoa::Buffer::new();
                app.out
                    .write(arg)
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
                if app.reverse_sorting {
                    ordering = ordering.reverse();
                }
                ordering
            });

            let dir = veneer::Directory::open(CStr::from_bytes(b".\0"))?;
            match app.display_mode {
                DisplayMode::Grid(width) => write_grid(&files, &dir, &mut app, width),
                DisplayMode::SingleColumn => write_single_column(&files, &dir, &mut app),
                DisplayMode::Stream => write_stream(&files, &dir, &mut app),
                DisplayMode::Long => unreachable!(),
            }
        } else {
            let mut files_and_stats = Vec::with_capacity(files.len());
            let dir = veneer::Directory::open(CStr::from_bytes(b".\0"))?;
            for e in files.iter().cloned() {
                let status = app.convert_status(syscalls::lstatat(dir.raw_fd(), e.name())?);
                files_and_stats.push((e, status));
            }

            if let Some(field) = app.sort_field {
                files_and_stats.sort_unstable_by(|a, b| {
                    let mut ordering = match field {
                        SortField::Time => {
                            b.1.time
                                .cmp(&a.1.time)
                                .then_with(|| vercmp(a.0.name(), b.0.name()))
                        }
                        SortField::Size => {
                            b.1.size
                                .cmp(&a.1.size)
                                .then_with(|| vercmp(a.0.name(), b.0.name()))
                        }
                        SortField::Name => vercmp(a.0.name(), b.0.name()),
                    };
                    if app.reverse_sorting {
                        ordering = ordering.reverse();
                    }
                    ordering
                });
            }

            match app.display_mode {
                DisplayMode::Grid(width) => write_grid(&files_and_stats, &dir, &mut app, width),
                DisplayMode::Long => write_details(&files_and_stats, &dir, &mut app),
                DisplayMode::SingleColumn => write_single_column(&files_and_stats, &dir, &mut app),
                DisplayMode::Stream => write_stream(&files_and_stats, &dir, &mut app),
            }
        }
    }

    if !dirs.is_empty() && !files.is_empty() {
        app.out.push(b'\n');
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        let contents = dir.read()?;
        let hint = contents.iter().size_hint();
        let mut entries: SmallVec<[veneer::directory::DirEntry; 32]> = SmallVec::new();
        entries.reserve(hint.1.unwrap_or(hint.0));

        match app.show_all {
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
            app.out.write(*name).write(b":\n");
        }

        if !need_details {
            if let Some(SortField::Name) = app.sort_field {
                entries.sort_unstable_by(|a, b| {
                    let mut ordering = vercmp(a.name(), b.name());
                    if app.reverse_sorting {
                        ordering = ordering.reverse();
                    }
                    ordering
                });
            }
            match app.display_mode {
                DisplayMode::Grid(width) => write_grid(&entries, &dir, &mut app, width),
                DisplayMode::SingleColumn => write_single_column(&entries, &dir, &mut app),
                DisplayMode::Stream => write_stream(&entries, &dir, &mut app),
                DisplayMode::Long => unreachable!(),
            }
        } else {
            let mut entries_and_stats = Vec::new();
            entries_and_stats.reserve(entries.len());
            for e in entries.iter().cloned() {
                let status = app.convert_status(syscalls::lstatat(dir.raw_fd(), e.name())?);
                entries_and_stats.push((e, status));
            }

            if let Some(field) = app.sort_field {
                entries_and_stats.sort_unstable_by(|a, b| {
                    let mut ordering = match field {
                        SortField::Time => {
                            b.1.time
                                .cmp(&a.1.time)
                                .then_with(|| vercmp(a.0.name(), b.0.name()))
                        }
                        SortField::Size => {
                            b.1.size
                                .cmp(&a.1.size)
                                .then_with(|| vercmp(a.0.name(), b.0.name()))
                        }
                        SortField::Name => vercmp(a.0.name(), b.0.name()),
                    };
                    if app.reverse_sorting {
                        ordering = ordering.reverse();
                    }
                    ordering
                });
            }

            match app.display_mode {
                DisplayMode::Grid(width) => write_grid(&entries_and_stats, &dir, &mut app, width),
                DisplayMode::Long => write_details(&entries_and_stats, &dir, &mut app),
                DisplayMode::SingleColumn => {
                    write_single_column(&entries_and_stats, &dir, &mut app)
                }
                DisplayMode::Stream => write_stream(&entries_and_stats, &dir, &mut app),
            }
        }

        if multiple_args && n != dirs.len() - 1 {
            app.out.push(b'\n');
        }
    }

    Ok(())
}

pub struct Status {
    pub links: libc::nlink_t,
    pub mode: libc::mode_t,
    pub size: libc::off_t,
    pub blocks: libc::blkcnt64_t,
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
    pub time: libc::time_t,
}

impl Status {
    fn style(&self) -> Option<Style> {
        let entry_type = self.mode & libc::S_IFMT;
        if entry_type == libc::S_IFDIR {
            Some(Style::BlueBold)
        } else if entry_type == libc::S_IFIFO {
            Some(Style::Yellow)
        } else if entry_type == libc::S_IFLNK {
            Some(Style::CyanBold)
        } else if self.mode & libc::S_IXUSR > 0 {
            Some(Style::GreenBold)
        } else {
            None
        }
    }
}
