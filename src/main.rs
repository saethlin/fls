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

#[macro_use]
mod output;

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
unsafe extern "C" fn main(argc: isize, argv: *const *const libc::c_char) -> i32 {
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

    if app.list_directory_contents {
        for arg in app.args.clone() {
            match veneer::Directory::open(arg) {
                Ok(d) => dirs.push((arg, d)),
                Err(Error(20)) => files.push(crate::directory::File { path: arg }),
                Err(e) => {
                    let mut buf = itoa::Buffer::new();
                    error!(
                        b"OS error ",
                        buf.format(e.0).as_bytes(),
                        b": ",
                        e.msg().as_bytes(),
                        b" \"",
                        arg.as_bytes(),
                        b"\"\n"
                    );
                }
            }
        }
    } else {
        for arg in app.args.clone() {
            files.push(crate::directory::File { path: arg })
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
                let status = if app.follow_symlinks == cli::FollowSymlinks::Always {
                    syscalls::fstatat(dir.raw_fd(), e.name())
                } else {
                    syscalls::lstatat(dir.raw_fd(), e.name())
                }
                .map(|status| app.convert_status(status))?;
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
        list_dir_contents(multiple_args, need_details, *name, dir, &mut app)?;
        // When recursing the recursion handles newlines, if not we need to check if we're on the
        // last and print a newline
        if !app.recurse && (n != dirs.len() - 1) {
            app.out.push(b'\n');
        }
    }

    Ok(())
}

fn list_dir_contents(
    multiple_args: bool,
    need_details: bool,
    name: CStr,
    dir: &veneer::Directory,
    app: &mut cli::App,
) -> Result<(), veneer::Error> {
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

    if multiple_args || app.recurse {
        print!(app, name, ":\n");
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
            DisplayMode::Grid(width) => write_grid(&entries, &dir, app, width),
            DisplayMode::SingleColumn => write_single_column(&entries, &dir, app),
            DisplayMode::Stream => write_stream(&entries, &dir, app),
            DisplayMode::Long => unreachable!(),
        }

        if app.recurse {
            app.out.push(b'\n');
            for e in entries.into_iter().filter(|e| e.is_dir() != Some(false)) {
                let mut path = name.as_bytes().to_vec();
                if path.last() != Some(&b'/') {
                    path.push(b'/');
                }
                path.extend_from_slice(e.name().as_bytes());
                path.push(0);
                let path = CStr::from_bytes(&path);
                if let Ok(dir) = veneer::Directory::open(path) {
                    list_dir_contents(multiple_args, need_details, path, &dir, app)?;
                }
            }
        }
    } else {
        let mut entries_and_stats = Vec::with_capacity(entries.len());
        for e in entries.iter().cloned() {
            let status = if app.follow_symlinks == cli::FollowSymlinks::Always {
                syscalls::fstatat(dir.raw_fd(), e.name())
            } else {
                syscalls::lstatat(dir.raw_fd(), e.name())
            }
            .map(|status| app.convert_status(status))?;
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
            DisplayMode::Grid(width) => write_grid(&entries_and_stats, &dir, app, width),
            DisplayMode::Long => write_details(&entries_and_stats, &dir, app),
            DisplayMode::SingleColumn => write_single_column(&entries_and_stats, &dir, app),
            DisplayMode::Stream => write_stream(&entries_and_stats, &dir, app),
        }

        if app.recurse {
            app.out.push(b'\n');
            for e in entries_and_stats.into_iter().filter_map(|(e, status)| {
                if status.mode & libc::S_IFMT == libc::S_IFDIR {
                    Some(e)
                } else {
                    None
                }
            }) {
                let mut path = name.as_bytes().to_vec();
                if path.last() != Some(&b'/') {
                    path.push(b'/');
                }
                path.extend_from_slice(e.name().as_bytes());
                path.push(0);
                let path = CStr::from_bytes(&path);
                if let Ok(dir) = veneer::Directory::open(path) {
                    list_dir_contents(multiple_args, need_details, path, &dir, app)?;
                }
            }
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
