use crate::directory::DirEntry;
use crate::syscalls;
use crate::{CStr, Error, Style};
use alloc::vec;
use alloc::vec::Vec;
use smallvec::{smallvec, SmallVec};

use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};

struct ShortStats {
    mode: u32,
    size: i64,
    uid: u32,
    mtime: i64,
}

pub fn write_details<T: DirEntry>(
    root: CStr,
    entries: &[T],
    out: &mut BufferedStdout,
) -> Result<(), Error> {
    let mut path = Vec::new();

    let mut all_stats: Vec<ShortStats> = Vec::with_capacity(entries.len());

    let mut longest_name_len = 0;
    let mut names = Vec::new();

    for e in entries {
        let mut stats: libc::stat64 = unsafe { core::mem::zeroed() };
        if e.name().get(0) != Some(&b'/') {
            path.clear();
            path.extend_from_slice(root.as_bytes());
            path.pop();
            path.push(b'/');
            path.extend_from_slice(e.name());
            path.push(0);
            syscalls::lstat(path.as_slice(), &mut stats)?;
        } else {
            syscalls::lstat(e.name(), &mut stats)?;
        }

        all_stats.push(ShortStats {
            mode: stats.st_mode,
            size: stats.st_size,
            uid: stats.st_uid,
            mtime: stats.st_mtime,
        });

        if !names.iter().any(|(uid, _)| *uid == stats.st_uid) {
            unsafe {
                let pw = libc::getpwuid(stats.st_uid);
                if !pw.is_null() {
                    let name_ptr = (*pw).pw_name;
                    let mut offset = 0;
                    let mut name: SmallVec<[u8; 24]> = SmallVec::new();
                    while *name_ptr.offset(offset) != 0 {
                        name.push(*name_ptr.offset(offset) as u8);
                        offset += 1;
                    }
                    longest_name_len = longest_name_len.max(name.len());
                    names.push((stats.st_uid, name));
                } else {
                    let mut buf = itoa::Buffer::new();
                    let mut name: SmallVec<[u8; 24]> = SmallVec::new();
                    name.extend_from_slice(buf.format(stats.st_uid).as_bytes());
                    longest_name_len = longest_name_len.max(name.len());
                    names.push((stats.st_uid, name));
                }
            }
        }
    }

    let current_year = unsafe {
        let mut localtime = core::mem::zeroed();
        let time = libc::time(core::ptr::null_mut());
        libc::localtime_r(&time, &mut localtime);
        localtime.tm_year
    };

    for (e, stats) in entries.iter().zip(all_stats.iter()) {
        let mode = stats.mode;

        let entry_type = mode & libc::S_IFMT;
        if entry_type == libc::S_IFDIR {
            out.style(Style::BlueBold)?;
            out.push(b'd')?;
        } else if entry_type == libc::S_IFLNK {
            out.style(Style::Cyan)?;
            out.push(b'l')?;
        } else {
            out.style(Style::White)?;
            out.push(b'.')?;
        }

        if mode & S_IRUSR > 0 {
            out.style(Style::YellowBold)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IWUSR > 0 {
            out.style(Style::RedBold)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IXUSR > 0 {
            out.style(Style::GreenBold)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IRGRP > 0 {
            out.style(Style::Yellow)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IWGRP > 0 {
            out.style(Style::Red)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IXGRP > 0 {
            out.style(Style::Green)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IROTH > 0 {
            out.style(Style::Yellow)?;
            out.push(b'r')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IWOTH > 0 {
            out.style(Style::Red)?;
            out.push(b'w')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        if mode & S_IXOTH > 0 {
            out.style(Style::Green)?;
            out.push(b'x')?;
        } else {
            out.style(Style::Gray)?;
            out.push(b'-')?;
        }

        out.push(b' ')?;
        out.style(Style::GreenBold)?;

        use libm::F32Ext;

        let gigabyte = 1024 * 1024 * 1024;
        let megabyte = 1024 * 1024;
        let kilobyte = 1024;

        if entry_type == libc::S_IFDIR {
            out.write(b"    ")?;
        } else if stats.size > gigabyte {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / gigabyte as f32;
            let size = buf
                .format(((stats.size as f32) / gigabyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]])?;
            } else if converted < 100. {
                out.push(b' ')?;
                out.write(&size[..2])?;
            } else {
                out.write(&size[..3])?;
            }
            out.push(b'G')?;
        } else if stats.size > 1_000_000 {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / megabyte as f32;
            let size = buf
                .format(((stats.size as f32) / megabyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]])?;
            } else if converted < 100. {
                out.push(b' ')?;
                out.write(&size[..2])?;
            } else {
                out.write(&size[..3])?;
            }
            out.push(b'M')?;
        } else if stats.size > 1_000 {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / kilobyte as f32;
            let size = buf
                .format(((stats.size as f32) / kilobyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]])?;
            } else if converted < 100. {
                out.push(b' ')?;
                out.write(&size[..2])?;
            } else {
                out.write(&size[..3])?;
            }
            out.push(b'K')?;
        } else {
            let mut buf = itoa::Buffer::new();
            let size = buf.format(stats.size);
            for _ in 0..(4 - size.len()) {
                out.push(b' ')?;
            }
            out.write(size.as_bytes())?;
        }
        out.push(b' ')?;

        out.style(Style::YellowBold)?;
        let name = names
            .iter()
            .find(|&(uid, _)| *uid == stats.uid)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        out.write(name.as_ref())?;
        // pad out to the length of the longest name
        if name.len() < longest_name_len {
            for _ in 0..longest_name_len - name.len() as usize {
                out.push(b' ')?;
            }
        }
        out.push(b' ')?;

        let mut localtime = libc::tm {
            tm_sec: 0,
            tm_min: 0,
            tm_hour: 0,
            tm_mday: 0,
            tm_mon: 0,
            tm_year: 0,
            tm_wday: 0,
            tm_yday: 0,
            tm_isdst: 0,
            tm_gmtoff: 0,
            tm_zone: core::ptr::null_mut(),
        };

        unsafe {
            libc::localtime_r(&stats.mtime, &mut localtime);
        };

        out.style(Style::Blue)?;

        out.write(month_abbr(localtime.tm_mon as u32))?;
        out.push(b' ')?;

        let mut buf = itoa::Buffer::new();
        let day = buf.format(localtime.tm_mday);
        if day.len() < 2 {
            out.push(b' ')?;
        }
        out.write(day.as_bytes())?;
        out.push(b' ')?;

        if localtime.tm_year == current_year {
            let hour = buf.format(localtime.tm_hour);
            if hour.len() < 2 {
                out.push(b' ')?;
            }
            out.write(hour.as_bytes())?;
            out.push(b':')?;

            let minute = buf.format(localtime.tm_min);
            if minute.len() < 2 {
                out.push(b'0')?;
            }
            out.write(minute.as_bytes())?;
        } else {
            out.push(b' ')?;
            out.write(buf.format(localtime.tm_year + 1900).as_bytes())?;
        }

        out.push(b' ')?;

        out.style(e.style()?)?;
        out.write(e.name())?;
        out.style(Style::Reset)?;
        out.push(b'\n')?;
    }
    Ok(())
}

fn len_utf8(s: &[u8]) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    if s.iter().all(|c| c.is_ascii()) {
        if s.last() == Some(&0) {
            s.len() - 1
        } else {
            s.len()
        }
    } else {
        core::str::from_utf8(s)
            .map(|s| s.graphemes(false).count())
            .unwrap_or_else(|_| s.len())
    }
}

#[inline(never)]
pub fn write_grid<T: DirEntry>(
    entries: &[T],
    out: &mut BufferedStdout,
    terminal_width: usize,
) -> Result<(), Error> {
    let mut rows = entries.len();
    let lengths: Vec<_> = entries.iter().map(|e| len_utf8(e.name())).collect();
    let mut widths: SmallVec<[usize; 16]> = match lengths.iter().max() {
        Some(max_len) => smallvec![*max_len],
        None => return Ok(()), // No entries, nothing to do here
    };

    for tmp_rows in (1..entries.len()).rev() {
        let mut tmp_widths: SmallVec<[usize; 16]> = SmallVec::new();
        for column in lengths.chunks(tmp_rows) {
            let width = column.iter().max().map(|m| *m).unwrap_or(1) + 2;
            tmp_widths.push(width);
        }
        tmp_widths.last_mut().map(|w| *w -= 2);
        if tmp_widths.iter().sum::<usize>() <= terminal_width {
            rows = tmp_rows;
            widths = tmp_widths;
        }
    }

    for r in 0..rows {
        for (c, width) in widths.iter().enumerate() {
            let (e, name_len) = match (entries.get(c * rows + r), lengths.get(c * rows + r)) {
                (Some(e), Some(name_len)) => (e, name_len),
                _ => continue,
            };

            out.style(e.style()?)?;
            out.write(e.name())?;

            for _ in 0..width - name_len {
                out.push(b' ')?;
            }
        }
        out.style(Style::Reset)?;
        out.push(b'\n')?;
    }
    Ok(())
}

pub fn write_single_column<T: DirEntry>(
    entries: &[T],
    out: &mut BufferedStdout,
) -> Result<(), Error> {
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
            style: Style::Reset,
        }
    }

    pub fn write<T: Writable>(&mut self, item: T) -> Result<(), Error> {
        for b in item.as_bytes() {
            if self.buf.try_push(*b).is_err() {
                write_to_stdout(&self.buf)?;
                self.buf.clear();
                self.buf.push(*b);
            }
        }
        Ok(())
    }

    pub fn push(&mut self, b: u8) -> Result<(), Error> {
        if self.buf.try_push(b).is_err() {
            write_to_stdout(&self.buf)?;
            self.buf.clear();
            self.buf.push(b);
        }
        Ok(())
    }

    pub fn style(&mut self, style: Style) -> Result<(), Error> {
        if self.style != style {
            self.write(style.to_bytes())?;
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

fn write_to_stdout(bytes: &[u8]) -> Result<(), Error> {
    let mut bytes_written = 0;
    while bytes_written < bytes.len() {
        bytes_written += syscalls::write(libc::STDOUT_FILENO, &bytes[bytes_written..])?;
    }
    Ok(())
}

fn month_abbr(month: u32) -> &'static [u8] {
    let month_names = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    if month < 12 {
        month_names[month as usize]
    } else {
        b"???"
    }
}
