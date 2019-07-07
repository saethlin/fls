use crate::directory::{Directory, Style};
use crate::Error;
use alloc::vec;
use alloc::vec::Vec;

const S_IXUSR: u32 = 64;
const S_IWUSR: u32 = 128;
const S_IRUSR: u32 = 256;
const S_IXGRP: u32 = 8;
const S_IWGRP: u32 = 16;
const S_IRGRP: u32 = 32;
const S_IXOTH: u32 = 1;
const S_IWOTH: u32 = 2;
const S_IROTH: u32 = 4;
const S_IFDIR: u32 = 16384;

pub fn write_details(root: &[u8], out: &mut BufferedStdout) -> Result<(), Error> {
    let mut entries = Vec::new();
    let dir = match Directory::open(root) {
        Ok(d) => d,
        Err(2) => return out.write(b"path doesn't exist (ENOENT)\n"),
        Err(13) => return out.write(b"access denied (EACCES)\n"),
        Err(20) => return out.write(b"path isn't a directory (ENOTDIR)\n"),
        Err(e) => return Err(e)?,
    };
    for e in dir.iter()?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    let mut path = root.to_vec();
    path.pop();
    path.push(b'/');

    for e in entries {
        path.extend_from_slice(e.name_with_nul());
        let mut stats = Stats::default();
        unsafe {
            let ret = syscall::syscall!(LSTAT, path.as_ptr(), (&mut stats) as *mut Stats) as isize;
            if ret < 0 {
                return Err(Error(-ret as i32));
            }
        }
        while path.last() != Some(&b'/') {
            path.pop();
        }
        let mode = stats.st_mode;

        if mode & S_IFDIR > 0 {
            out.style(Style::Directory)?;
            out.push(b'd')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'.')?;
        }

        if mode & S_IRUSR > 0 {
            out.style(Style::Yellow)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IWUSR > 0 {
            out.style(Style::BrokenSymlink)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IXUSR > 0 {
            out.style(Style::Executable)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IRGRP > 0 {
            out.style(Style::Yellow)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IWGRP > 0 {
            out.style(Style::BrokenSymlink)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IXGRP > 0 {
            out.style(Style::Executable)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IROTH > 0 {
            out.style(Style::Yellow)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IWOTH > 0 {
            out.style(Style::BrokenSymlink)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        if mode & S_IXOTH > 0 {
            out.style(Style::Executable)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Regular)?;
            out.push(b'-')?;
        }

        out.push(b' ')?;
        out.style(Style::Regular)?;

        use libm::F32Ext;

        let gigabyte = 1024 * 1024 * 1024;
        let megabyte = 1024 * 1024;
        let kilobyte = 1024;

        if mode & S_IFDIR > 0 {
            out.write(b"    ")?;
        } else if stats.st_size > gigabyte {
            let mut buf = itoa::Buffer::new();
            let size = buf
                .format(((stats.st_size as f32) / gigabyte as f32 * 10.).round() as u32)
                .as_bytes();
            out.write(&[size[0], b'.', size[1], b'G'])?;
        } else if stats.st_size > 1_000_000 {
            let mut buf = itoa::Buffer::new();
            let size = buf
                .format(((stats.st_size as f32) / megabyte as f32 * 10.).round() as u32)
                .as_bytes();
            out.write(&[size[0], b'.', size[1], b'M'])?;
        } else if stats.st_size > 1_000 {
            let mut buf = itoa::Buffer::new();
            let size = buf
                .format(((stats.st_size as f32) / kilobyte as f32 * 10.).round() as u32)
                .as_bytes();
            out.write(&[size[0], b'.', size[1], b'K'])?;
        } else {
            let mut buf = itoa::Buffer::new();
            let size = buf.format(stats.st_size);
            for _ in 0..(4 - size.len()) {
                out.push(b' ')?;
            }
            out.write(size.as_bytes())?;
        }
        out.push(b' ')?;

        out.style(e.style()?)?;
        out.write(e.name())?;
        out.style(Style::Regular)?;
        out.push(b'\n')?;
    }
    Ok(())
}

#[repr(C)]
#[derive(Default)]
struct Stats {
    st_dev: u64,
    st_ino: u64,
    st_nlink: u64,

    st_mode: u32,
    st_uid: u32,
    st_gid: u32,
    _padding: u32,
    st_rdev: u64,
    st_size: u64,
    st_blksize: u32,
    st_blocks: u64,

    st_atim: Timespec,
    st_mtim: Timespec,
    st_ctim: Timespec,
    _unused: [u32; 3],
}

#[repr(C)]
#[derive(Default)]
struct Timespec {
    tv_sec: u32,
    tv_nsec: u32,
}

pub fn write_grid(
    root: &[u8],
    out: &mut BufferedStdout,
    terminal_width: usize,
) -> Result<(), Error> {
    let mut entries = Vec::new();
    let dir = match Directory::open(root) {
        Ok(d) => d,
        Err(2) => return out.write(b"path doesn't exist (ENOENT)\n"),
        Err(13) => return out.write(b"access denied (EACCES)\n"),
        Err(20) => return out.write(b"path isn't a directory (ENOTDIR)\n"),
        Err(e) => return Err(e)?,
    };
    for e in dir.iter()?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    let mut columns = 1;
    let mut widths = match entries.iter().map(|e| e.name().len()).max() {
        Some(max_len) => vec![max_len],
        None => return Ok(()), // No entries, nothing to do here
    };
    for n_columns in 1..=entries.len() {
        let mut tmp_widths = vec![0; n_columns];
        let entries_per_column =
            entries.len() / n_columns + ((entries.len() % n_columns != 0) as usize);
        for (c, column) in entries.chunks(entries_per_column).enumerate() {
            tmp_widths[c] = column
                .iter()
                .map(|entry| entry.name().len())
                .max()
                .unwrap_or(1)
                + 2;
        }
        if tmp_widths.iter().sum::<usize>() > terminal_width as usize {
            break;
        } else {
            columns = n_columns;
            widths = tmp_widths;
        }
    }

    let rows = entries.len() / columns + ((entries.len() % columns != 0) as usize);

    for r in 0..rows {
        for (c, width) in widths.iter().enumerate() {
            let e = match entries.get(c * rows + r) {
                Some(e) => e,
                None => break,
            };

            out.style(e.style()?)?;
            out.write(e.name())?;

            for _ in 0..width - e.name().len() {
                out.push(b' ')?;
            }
        }
        out.push(b'\n')?;
    }
    out.style(Style::Regular)?;

    Ok(())
}

pub fn write_single_column(root: &[u8], out: &mut BufferedStdout) -> Result<(), Error> {
    let mut entries = Vec::new();
    let dir = Directory::open(root)?;
    for e in dir.iter()?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    for e in entries {
        out.write(e.name())?;
        out.push(b'\n')?;
    }
    Ok(())
}

pub trait Writable {
    fn as_bytes(&self) -> &[u8];
}

impl Writable for &[u8] {
    fn as_bytes(&self) -> &[u8] {
        *self
    }
}

macro_rules! array_impl {
    ($($N:expr)+) => {
        $(
            impl Writable for &[u8; $N] {
                fn as_bytes(&self) -> &[u8] {
                    *self
                }
            }
        )+
    }
}

array_impl! {
     0  1  2  3  4  5  6  7  8  9
    10 11 12 13 14 15 16 17 18 19
    20 21 22 23 24 25 26 27 28 29
    30 31 32 33
}

pub struct BufferedStdout {
    buf: arrayvec::ArrayVec<[u8; 4096]>,
    style: Style,
}

impl BufferedStdout {
    pub fn new() -> Self {
        Self {
            buf: arrayvec::ArrayVec::new(),
            style: Style::Regular,
        }
    }

    pub fn write<T: Writable>(&mut self, item: T) -> Result<(), Error> {
        for b in item.as_bytes() {
            if let Err(_) = self.buf.try_push(*b) {
                write_to_stdout(&self.buf)?;
                self.buf.clear();
                self.buf.push(*b);
            }
        }
        Ok(())
    }

    pub fn push(&mut self, b: u8) -> Result<(), Error> {
        if let Err(_) = self.buf.try_push(b) {
            write_to_stdout(&self.buf)?;
            self.buf.clear();
            self.buf.push(b);
        }
        Ok(())
    }

    pub fn style(&mut self, style: Style) -> Result<(), Error> {
        if self.style != style {
            let bytes: &[u8] = match style {
                Style::Regular => b"\x1B[m",
                Style::Directory => b"\x1B[1;34m",
                Style::Executable => b"\x1B[1;32m",
                Style::Symlink => b"\x1B[0;36m",
                Style::BrokenSymlink => b"\x1B[0;31m",
                Style::Compressed => b"\x1B[0;31m",
                Style::Document => b"\x1B[0;38;5;105m",
                Style::Media => b"\x1B[0;38;5;133m",
                Style::Yellow => b"\x1B[1;33m",
            };
            self.write(bytes)?;
            self.style = style;
        }
        Ok(())
    }
}

impl Drop for BufferedStdout {
    fn drop(&mut self) {
        let _ = write_to_stdout(&self.buf);
    }
}

fn write_to_stdout(bytes: &[u8]) -> Result<(), i32> {
    unsafe {
        let mut bytes_written = 0;
        while bytes_written < bytes.len() {
            let ret = syscall::syscall!(
                WRITE,
                1,
                bytes.as_ptr().offset(bytes_written as isize),
                bytes.len() - bytes_written
            ) as i32;
            if ret < 0 {
                return Err(-ret);
            } else {
                bytes_written += ret as usize;
            }
        }
    }
    Ok(())
}
