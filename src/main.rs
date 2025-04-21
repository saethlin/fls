#![no_std]
#![no_main]

extern crate alloc;

mod cli;
mod directory;
mod output;
mod style;
mod time;
mod utils;

use crate::{
    cli::{App, Args, DisplayMode, ShowAll, SortField},
    output::*,
    style::Style,
};
use alloc::vec::Vec;
use veneer::{
    fs::{DType, DirEntry, Directory},
    libc, syscalls, CStr, Error,
};

#[veneer::main]
fn main() -> Result<(), Error> {
    let mut app = App::DEFAULT;
    app.init()?;

    if matches!(app.args, Args::None) && !app.recurse {
        let dir = Directory::open(CStr::from_bytes(b".\0"))?;
        list_dir_contents(&mut Vec::new(), &mut Vec::new(), &dir, &mut app);
        return Ok(());
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    if app.list_directory_contents {
        for arg in app.args.iter() {
            match Directory::open(arg) {
                Ok(d) => dirs.push((arg, d)),
                Err(Error(20)) => files.push(DirEntry {
                    name: arg,
                    inode: 0,
                    d_type: DType::UNKNOWN,
                }),
                Err(_) => {
                    if let Err(err) = veneer::syscalls::fstatat(libc::AT_FDCWD, arg) {
                        access_error(&arg, err);
                    } else {
                        files.push(DirEntry {
                            name: arg,
                            inode: 0,
                            d_type: DType::UNKNOWN,
                        });
                    }
                }
            }
        }
    } else {
        for arg in app.args.iter() {
            files.push(DirEntry {
                name: arg,
                inode: 0,
                d_type: DType::UNKNOWN,
            });
        }
    }

    // FIXME
    /*
    if !files.is_empty() {
        let dir = Directory::open(CStr::from_bytes(b".\0")).unwrap();
        if app.needs_details {
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
                        access_error(&e.name(), err);
                    }
                }
            }
        }

        sort_entries(&mut files, &app);

        match app.display_mode {
            DisplayMode::Grid(width) => write_grid(&files, &dir, &mut app, width),
            DisplayMode::Long => write_details(&files, &dir, &mut app),
            DisplayMode::SingleColumn => write_single_column(&files, &dir, &mut app),
            DisplayMode::Stream => write_stream(&files, &dir, &mut app),
        }
    }
    */

    if !dirs.is_empty() && !files.is_empty() {
        app.out.push(b'\n');
    }

    for (n, (name, dir)) in dirs.iter().enumerate() {
        let mut path = Vec::new();
        path.extend(name.as_bytes());
        let status = syscalls::fstat(dir.raw_fd()).unwrap();
        let mut stack = Vec::new();
        if app.recurse {
            stack.push((status.st_dev, status.st_ino));
        }
        list_dir_contents(&mut stack, &mut path, dir, &mut app);
        // When recursing the recursion handles newlines, if not we need to check if we're on the
        // last and print a newline
        if !app.recurse && (n != dirs.len() - 1) {
            app.out.push(b'\n');
        }
    }

    Ok(())
}

use veneer::fs::BorrowedDirectoryContents;
fn sort_entries(entries: &mut BorrowedDirectoryContents, details: &[Status], app: &App) {
    let Some(field) = app.sort_field else {
        return;
    };

    entries.sort_unstable_by(|(i_a, a), (i_b, b)| {
        let mut ordering = match field {
            SortField::Name => vercmp(a.name(), b.name()),
            SortField::Time => {
                let a_details = &details[i_a];
                let b_details = &details[i_b];
                b_details
                    .time
                    .cmp(&a_details.time)
                    .then_with(|| vercmp(a.name(), b.name()))
            }
            SortField::Size => {
                let a_details = &details[i_a];
                let b_details = &details[i_b];
                b_details
                    .size
                    .cmp(&a_details.size)
                    .then_with(|| vercmp(a.name(), b.name()))
            }
        };
        if app.reverse_sorting {
            ordering = ordering.reverse();
        }
        ordering
    });
}

fn is_visible(app: &mut App, name: &[u8]) -> bool {
    match app.show_all {
        ShowAll::No => name.get(0) != Some(&b'.'),
        ShowAll::Almost => !(name == b".\0" || name == b"..\0"),
        ShowAll::Yes => true,
    }
}

fn list_dir_contents(
    stack: &mut Vec<(libc::dev_t, libc::ino_t)>,
    path: &mut Vec<u8>,
    dir: &Directory,
    app: &mut App,
) {
    let mut contents = match dir.read(|entry| is_visible(app, entry)) {
        Ok(c) => c,
        Err(err) => {
            access_error(path, err);
            return;
        }
    };

    let mut entries = contents.index();

    if matches!(app.args, Args::Multiple) || app.recurse {
        if path.len() > 1 && path.last() == Some(&0) {
            path.pop();
        }
        if path.len() > 1 && path.last() == Some(&b'/') {
            path.pop();
        }
        app.out.write(path).write(b":\n");
    }

    let mut details = Vec::new();
    if app.needs_details {
        details.reserve(entries.len());
        for e in entries.iter() {
            let status = if app.follow_symlinks == cli::FollowSymlinks::Always {
                syscalls::fstatat(dir.raw_fd(), e.name())
            } else {
                syscalls::lstatat(dir.raw_fd(), e.name())
            }
            .map(|status| app.convert_status(status));
            let status = match status {
                Ok(s) => s,
                Err(err) => {
                    access_error(&e.name(), err);
                    Status::default()
                }
            };
            details.push(status);
        }
    };

    sort_entries(&mut entries, &details, app);

    match app.display_mode {
        DisplayMode::Grid(width) => write_grid(&entries, &details, dir, app, width),
        DisplayMode::Long => write_details(&entries, &details, dir, app),
        DisplayMode::SingleColumn => write_single_column(&entries, &details, dir, app),
        DisplayMode::Stream => write_stream(&entries, &details, dir, app),
    }
    app.out.flush();

    if app.recurse {
        app.out.push(b'\n');
        for (i, entry) in entries.iter_enumerated() {
            // If we have stat details for the entry, skip over if it we know it's not a
            // directory.
            if let Some(st) = details.get(i) {
                if st.mode & libc::S_IFMT != libc::S_IFDIR {
                    continue;
                }
            }
            // Skip the magic . and .. entries
            if entry.name.as_bytes() == b".." || entry.name.as_bytes() == b"." {
                continue;
            }

            match entry.d_type {
                DType::DIR | DType::UNKNOWN => {}
                _ => continue,
            }

            if path.last() == Some(&0) {
                path.pop();
            }
            if path.last() != Some(&b'/') {
                path.push(b'/');
            }
            path.extend(entry.name.as_bytes());
            path.push(0);
            match Directory::open(CStr::from_bytes(path)) {
                Ok(dir) => {
                    let status = syscalls::fstat(dir.raw_fd()).unwrap();
                    if !stack.contains(&(status.st_dev, status.st_ino)) {
                        stack.push((status.st_dev, status.st_ino));
                        list_dir_contents(stack, path, &dir, app);
                        stack.pop();
                    }
                }
                Err(err) => {
                    access_error(&path[..path.len() - 1], err);
                }
            }
            while path.last() != Some(&b'/') {
                path.pop();
            }
        }
    }
    if path.last() == Some(&b'/') {
        path.pop();
    }
}

#[inline(never)]
fn access_error(item: &[u8], error: Error) {
    let mut out = crate::output::OutputBuffer::to_fd(2);
    out.write(&b"Unable to access '"[..])
        .write(item)
        .write(&b"': OS Error "[..]);
    error.0.write(&mut out);
    out.push(b'\n');
}

#[derive(Default, Clone)]
pub struct Status {
    pub device: libc::dev_t,
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
