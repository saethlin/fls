#![no_main]
#![no_std]
#![feature(alloc_error_handler)]
#![allow(
    clippy::enum_glob_use,
    clippy::option_if_let_else,
    clippy::unseparated_literal_suffix,
    clippy::too_many_lines
)]

macro_rules! error {
    ($($item:expr),+) => {
        {
            use crate::output::Writable;
            let mut buf = crate::output::OutputBuffer::to_fd(2);
            $($item.write(&mut buf);)*
            buf.flush();
        }
    };
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "Panicked at ",
            location.file(),
            ":",
            location.line() as u64,
            "\n"
        );
    } else {
        let _ = error!("Panicked, location unknown\n");
    }
    let _ = veneer::syscalls::kill(0, libc::SIGABRT);
    veneer::syscalls::exit(-1);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    error!(
        "Unable to allocate, size: ",
        crate::utils::Buffer::new().format(layout.size() as u64),
        "\n"
    );
    let _ = veneer::syscalls::kill(0, libc::SIGABRT);
    veneer::syscalls::exit(-1);
    loop {}
}

#[global_allocator]
static ALLOC: veneer::Allocator = veneer::Allocator;

extern crate alloc;
use alloc::vec::Vec;

// Temporariliy pasting veneer's code into this crate until veneer is more done
mod veneer;

#[macro_use]
pub mod output;
mod cli;
mod directory;
mod error;
mod style;
mod utils;

use cli::{DisplayMode, ShowAll, SortField};
use directory::DirEntryExt;
use output::*;
use style::Style;

use core::sync::atomic::{AtomicI32, Ordering::Relaxed};
use veneer::{directory::DType, syscalls, CStr, Error};

static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

#[no_mangle]
unsafe extern "C" fn main(argc: isize, argv: *const *const libc::c_char) -> i32 {
    let arguments = (0..argc).map(|i| CStr::from_ptr(*argv.offset(i))).collect();

    match run(arguments) {
        Ok(()) => LAST_ERROR.load(Relaxed),
        Err(e) => e.0 as i32,
    }
}

fn run(args: Vec<CStr<'static>>) -> Result<(), Error> {
    let mut app = cli::App::from_arguments(args)?;

    let need_details = app.display_mode == DisplayMode::Long
        || app.sort_field == Some(SortField::Time)
        || app.sort_field == Some(SortField::Size)
        || app.display_size_in_blocks;

    let multiple_args = app.args.len() > 1;

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut pool = crate::output::Pool::default();

    if app.list_directory_contents {
        for arg in app.args.clone() {
            match veneer::Directory::open(arg) {
                Ok(d) => dirs.push((arg, d)),
                Err(Error(20)) => files.push((
                    crate::veneer::directory::DirEntry {
                        name: arg,
                        inode: 0,
                        d_type: DType::UNKNOWN,
                    },
                    None,
                )),
                Err(_) => {
                    if let Err(e) = veneer::syscalls::stat(arg) {
                        let mut buf = crate::utils::Buffer::new();
                        error!(
                            "OS error ",
                            buf.format(e.0 as u64),
                            ": ",
                            e.msg().as_bytes(),
                            " \"",
                            arg.as_bytes(),
                            "\""
                        );
                        LAST_ERROR.store(e.0, Relaxed);
                    } else {
                        files.push((
                            crate::veneer::directory::DirEntry {
                                name: arg,
                                inode: 0,
                                d_type: DType::UNKNOWN,
                            },
                            None,
                        ));
                    }
                }
            }
        }
    } else {
        for arg in app.args.clone() {
            files.push((
                crate::veneer::directory::DirEntry {
                    name: arg,
                    inode: 0,
                    d_type: DType::UNKNOWN,
                },
                None,
            ));
        }
    }

    if !files.is_empty() {
        let dir = veneer::Directory::open(CStr::from_bytes(b".\0")).unwrap();
        if need_details {
            for e in &mut files {
                let status = if app.follow_symlinks == cli::FollowSymlinks::Always {
                    syscalls::fstatat(dir.raw_fd(), e.name())
                } else {
                    syscalls::lstatat(dir.raw_fd(), e.name())
                }
                .map(|status| app.convert_status(status));
                match status {
                    Ok(s) => e.1 = Some(s),
                    Err(err) => {
                        error!(
                            b"Unable to access '",
                            e.name().as_bytes(),
                            b"': ",
                            err.msg().as_bytes()
                        );
                        LAST_ERROR.store(err.0, Relaxed);
                    }
                }
            }
        }

        sort_entries(&mut files, &app);

        match app.display_mode {
            DisplayMode::Grid(width) => write_grid(&files, &dir, &mut app, width, &mut pool),
            DisplayMode::Long => write_details(&files, &dir, &mut app),
            DisplayMode::SingleColumn => write_single_column(&files, &dir, &mut app),
            DisplayMode::Stream => write_stream(&files, &dir, &mut app),
        }
    }

    if !dirs.is_empty() && !files.is_empty() {
        app.out.push(b'\n');
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        list_dir_contents(multiple_args, need_details, *name, dir, &mut app, &mut pool);
        // When recursing the recursion handles newlines, if not we need to check if we're on the
        // last and print a newline
        if !app.recurse && (n != dirs.len() - 1) {
            app.out.push(b'\n');
        }
    }

    Ok(())
}

use crate::cli::App;
use crate::veneer::DirEntry;
fn sort_entries(entries: &mut [(DirEntry, Option<Status>)], app: &App) {
    if let Some(field) = app.sort_field {
        entries.sort_unstable_by(|a, b| {
            let mut ordering = match field {
                SortField::Time => b
                    .time()
                    .cmp(&a.time())
                    .then_with(|| vercmp(a.name(), b.name())),
                SortField::Size => {
                    b.1.clone()
                        .unwrap_or_default()
                        .size
                        .cmp(&a.1.clone().unwrap_or_default().size)
                        .then_with(|| vercmp(a.name(), b.name()))
                }
                SortField::Name => vercmp(a.name(), b.name()),
            };
            if app.reverse_sorting {
                ordering = ordering.reverse();
            }
            ordering
        });
    }
}

fn list_dir_contents(
    multiple_args: bool,
    need_details: bool,
    name: CStr,
    dir: &veneer::Directory,
    app: &mut cli::App,
    pool: &mut Pool,
) {
    let contents = match dir.read() {
        Ok(c) => c,
        Err(err) => {
            error!(
                b"Unable to access '",
                name.as_bytes(),
                b"': ",
                err.msg().as_bytes()
            );
            LAST_ERROR.store(err.0, Relaxed);
            return;
        }
    };
    let hint = contents.iter().size_hint();
    let mut entries = Vec::new();
    entries.reserve(hint.1.unwrap_or(hint.0));

    match app.show_all {
        ShowAll::No => {
            for e in contents.iter().filter(|e| e.name.get(0) != Some(b'.')) {
                entries.push((e, None));
            }
        }
        ShowAll::Almost => {
            for e in contents.iter() {
                if e.name.as_bytes() != b".." && e.name.as_bytes() != b"." {
                    entries.push((e, None));
                }
            }
        }
        ShowAll::Yes => {
            for e in contents.iter() {
                entries.push((e, None));
            }
        }
    }

    if multiple_args || app.recurse {
        print!(app, name, ":\n");
    }

    if need_details {
        for e in &mut entries {
            let status = if app.follow_symlinks == cli::FollowSymlinks::Always {
                syscalls::fstatat(dir.raw_fd(), e.name())
            } else {
                syscalls::lstatat(dir.raw_fd(), e.name())
            }
            .map(|status| app.convert_status(status));
            match status {
                Ok(s) => e.1 = Some(s),
                Err(err) => {
                    error!(
                        b"Unable to access '",
                        e.name().as_bytes(),
                        b"': ",
                        err.msg().as_bytes(),
                        b"\n"
                    );
                    LAST_ERROR.store(err.0, Relaxed);
                }
            }
        }
    }

    sort_entries(&mut entries, app);

    match app.display_mode {
        DisplayMode::Grid(width) => write_grid(&entries, dir, app, width, pool),
        DisplayMode::Long => write_details(&entries, dir, app),
        DisplayMode::SingleColumn => write_single_column(&entries, dir, app),
        DisplayMode::Stream => write_stream(&entries, dir, app),
    }

    if app.recurse {
        app.out.push(b'\n');
        let mut path = Vec::new();
        for e in entries
            .into_iter()
            .filter_map(|(e, status)| {
                if let Some(st) = status {
                    if st.mode & libc::S_IFMT == libc::S_IFDIR {
                        Some(e)
                    } else {
                        None
                    }
                } else {
                    Some(e)
                }
            })
            .filter(|e| e.name.as_bytes() != b"..")
            .filter(|e| e.name.as_bytes() != b".")
            .filter(|e| e.d_type == DType::DIR || e.d_type == DType::UNKNOWN)
        {
            path.clear();
            path.extend(name.as_bytes());
            if path.last() != Some(&b'/') {
                path.push(b'/');
            }
            path.extend_from_slice(e.name.as_bytes());
            path.push(0);
            let path = CStr::from_bytes(&path);
            if let Ok(dir) = veneer::Directory::open(path) {
                app.out.flush();
                list_dir_contents(multiple_args, need_details, path, &dir, app, pool);
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct Status {
    pub links: libc::nlink_t,
    pub mode: libc::mode_t,
    pub size: libc::off_t,
    pub blocks: libc::blkcnt64_t,
    pub block_size: libc::blksize_t,
    pub uid: libc::uid_t,
    pub gid: libc::gid_t,
    pub time: libc::time_t,
    pub inode: libc::ino_t,
}
