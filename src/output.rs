use crate::directory::DirEntry;
use crate::{Status, Style};
use alloc::vec;
use alloc::vec::Vec;
use smallvec::{smallvec, SmallVec};

use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};
use unicode_segmentation::UnicodeSegmentation;
use veneer::syscalls;

pub fn write_details<T: DirEntry>(
    entries: &[(T, Status)],
    uid_usernames: &mut Vec<((u32, SmallVec<[u8; 24]>))>,
    out: &mut BufferedStdout,
) {
    let mut longest_name_len = 0;
    for (_, stats) in entries {
        if !uid_usernames.iter().any(|(uid, _)| *uid == stats.uid) {
            unsafe {
                let pw = libc::getpwuid(stats.uid);
                if !pw.is_null() {
                    let name_ptr = (*pw).pw_name;
                    let mut offset = 0;
                    let mut name: SmallVec<[u8; 24]> = SmallVec::new();
                    while *name_ptr.offset(offset) != 0 {
                        name.push(*name_ptr.offset(offset) as u8);
                        offset += 1;
                    }
                    longest_name_len = longest_name_len.max(name.len());
                    uid_usernames.push((stats.uid, name));
                } else {
                    let mut buf = itoa::Buffer::new();
                    let mut name: SmallVec<[u8; 24]> = SmallVec::new();
                    name.extend_from_slice(buf.format(stats.uid).as_bytes());
                    longest_name_len = longest_name_len.max(name.len());
                    uid_usernames.push((stats.uid, name));
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

    for (e, stats) in entries {
        let mode = stats.mode;

        let entry_type = mode & libc::S_IFMT;
        if entry_type == libc::S_IFDIR {
            out.style(Style::BlueBold).push(b'd');
        } else if entry_type == libc::S_IFLNK {
            out.style(Style::Cyan).push(b'l');
        } else {
            out.style(Style::White).push(b'.');
        }

        if mode & S_IRUSR > 0 {
            out.style(Style::YellowBold).push(b'r');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IWUSR > 0 {
            out.style(Style::RedBold).push(b'w');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IXUSR > 0 {
            out.style(Style::GreenBold).push(b'x');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IRGRP > 0 {
            out.style(Style::Yellow).push(b'r');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IWGRP > 0 {
            out.style(Style::Red).push(b'w');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IXGRP > 0 {
            out.style(Style::Green).push(b'x');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IROTH > 0 {
            out.style(Style::Yellow).push(b'r');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IWOTH > 0 {
            out.style(Style::Red).push(b'w');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        if mode & S_IXOTH > 0 {
            out.style(Style::Green).push(b'x');
        } else {
            out.style(Style::Gray).push(b'-');
        }

        out.push(b' ').style(Style::GreenBold);

        use libm::F32Ext;

        let gigabyte = 1024 * 1024 * 1024;
        let megabyte = 1024 * 1024;
        let kilobyte = 1024;

        if entry_type == libc::S_IFDIR {
            out.write(b"    ");
        } else if stats.size > gigabyte {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / gigabyte as f32;
            let size = buf
                .format(((stats.size as f32) / gigabyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]]);
            } else if converted < 100. {
                out.push(b' ');
                out.write(&size[..2]);
            } else {
                out.write(&size[..3]);
            }
            out.push(b'G');
        } else if stats.size > 1_000_000 {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / megabyte as f32;
            let size = buf
                .format(((stats.size as f32) / megabyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]]);
            } else if converted < 100. {
                out.push(b' ');
                out.write(&size[..2]);
            } else {
                out.write(&size[..3]);
            }
            out.push(b'M');
        } else if stats.size > 1_000 {
            let mut buf = itoa::Buffer::new();
            let converted = (stats.size as f32) / kilobyte as f32;
            let size = buf
                .format(((stats.size as f32) / kilobyte as f32 * 10.).round() as u32)
                .as_bytes();
            if converted < 10. {
                out.write(&[size[0], b'.', size[1]]);
            } else if converted < 100. {
                out.push(b' ');
                out.write(&size[..2]);
            } else {
                out.write(&size[..3]);
            }
            out.push(b'K');
        } else {
            let mut buf = itoa::Buffer::new();
            let size = buf.format(stats.size);
            for _ in 0..(4 - size.len()) {
                out.push(b' ');
            }
            out.write(size);
        }
        out.push(b' ');

        out.style(Style::YellowBold);
        let name = uid_usernames
            .iter()
            .find(|&(uid, _)| *uid == stats.uid)
            .map(|(_, name)| name.clone())
            .unwrap_or_default();
        out.write(name.as_ref());
        // pad out to the length of the longest name
        if name.len() < longest_name_len {
            for _ in 0..longest_name_len - name.len() as usize {
                out.push(b' ');
            }
        }
        out.push(b' ');

        let localtime = unsafe {
            let mut localtime = core::mem::zeroed();
            libc::localtime_r(&stats.mtime, &mut localtime);
            localtime
        };

        out.style(Style::Blue)
            .write(month_abbr(localtime.tm_mon as u32))
            .push(b' ');

        let mut buf = itoa::Buffer::new();
        let day = buf.format(localtime.tm_mday);
        if day.len() < 2 {
            out.push(b' ');
        }
        out.write(day).push(b' ');

        if localtime.tm_year == current_year {
            let hour = buf.format(localtime.tm_hour);
            if hour.len() < 2 {
                out.push(b' ');
            }
            out.write(hour);
            out.push(b':');

            let minute = buf.format(localtime.tm_min);
            if minute.len() < 2 {
                out.push(b'0');
            }
            out.write(minute);
        } else {
            out.push(b' ');
            out.write(buf.format(localtime.tm_year + 1900));
        }

        out.push(b' ');

        out.style(
            stats
                .style()
                .unwrap_or_else(|| crate::directory::extension_style(e.name().as_bytes())),
        );
        out.write(e.name());
        out.style(Style::Reset);
        out.push(b'\n');
    }
}

pub fn write_grid<T: DirEntry>(
    entries: &[T],
    dir: &veneer::Directory,
    out: &mut BufferedStdout,
    terminal_width: usize,
) {
    if entries.is_empty() {
        return;
    }

    let mut rows = entries.len();
    let mut lengths: Vec<usize> = Vec::with_capacity(entries.len());
    let mut min_len: usize = len_utf8(entries[0].name().as_bytes());
    for e in entries {
        let len = len_utf8(e.name().as_bytes());
        lengths.push(len);
        min_len = min_len.min(len);
    }
    let mut widths: SmallVec<[usize; 16]> = lengths
        .iter()
        .max()
        .map(|max_len| smallvec![*max_len])
        .unwrap();

    let max_columns = terminal_width / min_len;

    for tmp_rows in (1..entries.len()).rev() {
        let mut tmp_widths: SmallVec<[usize; 16]> = SmallVec::new();
        for column in lengths.chunks(tmp_rows) {
            let width = column.iter().max().copied().unwrap_or(1) + 2;
            tmp_widths.push(width);
        }
        // Try to exit early if we're in a huge directory
        if tmp_widths.len() > max_columns {
            break;
        }

        if let Some(w) = tmp_widths.last_mut() {
            *w -= 2;
        }
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

            out.style(e.style(dir)).write(e.name());

            for _ in 0..(width - name_len) {
                out.push(b' ');
            }
        }
        out.style(Style::Reset);
        out.push(b'\n');
    }
}

pub fn write_single_column<T: DirEntry>(entries: &[T], out: &mut BufferedStdout) {
    for e in entries {
        out.write(e.name()).push(b'\n');
    }
}

fn len_utf8(bytes: &[u8]) -> usize {
    core::str::from_utf8(bytes)
        .map(|s| s.graphemes(false).count())
        .unwrap_or_else(|_| bytes.len())
}

pub trait Writable {
    fn bytes_repr(&self) -> &[u8];
}

impl Writable for &[u8] {
    fn bytes_repr(&self) -> &[u8] {
        *self
    }
}

macro_rules! array_impl {
    ($($N:expr)+) => {
        $(
            impl Writable for &[u8; $N] {
                fn bytes_repr(&self) -> &[u8] {
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

impl Writable for &str {
    fn bytes_repr(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a> Writable for veneer::CStr<'a> {
    fn bytes_repr(&self) -> &[u8] {
        self.as_bytes()
    }
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

    pub fn write<T: Writable>(&mut self, item: T) -> &mut Self {
        for b in item.bytes_repr() {
            if self.buf.try_push(*b).is_err() {
                write_to_stdout(&self.buf);
                self.buf.clear();
                self.buf.push(*b);
            }
        }
        self
    }

    pub fn push(&mut self, b: u8) -> &mut Self {
        if self.buf.try_push(b).is_err() {
            write_to_stdout(&self.buf);
            self.buf.clear();
            self.buf.push(b);
        }
        self
    }

    pub fn style(&mut self, style: Style) -> &mut Self {
        if self.style != style {
            self.write(style.to_bytes());
            self.style = style;
        }
        self
    }
}

impl Drop for BufferedStdout {
    fn drop(&mut self) {
        write_to_stdout(&self.buf);
    }
}

fn write_to_stdout(bytes: &[u8]) {
    let mut bytes_written = 0;
    while bytes_written < bytes.len() {
        bytes_written += syscalls::write(libc::STDOUT_FILENO, &bytes[bytes_written..]).unwrap();
    }
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

use core::cmp::Ordering;
pub fn vercmp(s1_cstr: veneer::CStr, s2_cstr: veneer::CStr) -> Ordering {
    let s1 = s1_cstr.as_bytes();
    let s2 = s2_cstr.as_bytes();
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
