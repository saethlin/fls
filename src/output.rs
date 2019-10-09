use crate::cli::App;
use crate::directory::DirEntry;
use crate::{Status, Style};
use alloc::vec;
use alloc::vec::Vec;
use smallvec::{smallvec, SmallVec};

use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};
use unicode_segmentation::UnicodeSegmentation;
use veneer::syscalls;

macro_rules! print {
    ($app:expr, $($item:expr),+) => {
        {
        $($item.write(&mut $app.out);)*
    }};
}

macro_rules! error {
    ($($item:expr),+) => {
        {
        use alloc::vec::Vec;
        let mut err = Vec::new();
        err.extend_from_slice(b"fls: ");
        $(err.extend($item);)*
        let _ = veneer::syscalls::write(2, &err[..]);
    }};
}

pub fn write_details<T: DirEntry>(entries: &[(T, Status)], dir: &veneer::Directory, app: &mut App) {
    use Style::*;

    let get_name = |id: libc::uid_t| unsafe {
        core::ptr::NonNull::new(libc::getpwuid(id)).map(|pw| {
            let mut name: SmallVec<[u8; 24]> = SmallVec::new();
            let name_ptr = pw.as_ref().pw_name;
            let mut offset = 0;
            while *name_ptr.offset(offset) != 0 {
                name.push(*name_ptr.offset(offset) as u8);
                offset += 1;
            }
            name
        })
    };

    let get_group = |id: libc::gid_t| unsafe {
        core::ptr::NonNull::new(libc::getgrgid(id)).map(|gr| {
            let mut group: SmallVec<[u8; 24]> = SmallVec::new();
            let group_ptr = gr.as_ref().gr_name;
            let mut offset = 0;
            while *group_ptr.offset(offset) != 0 {
                group.push(*group_ptr.offset(offset) as u8);
                offset += 1;
            }
            group
        })
    };

    let mut longest_name_len = 0;
    let mut longest_group_len = 0;
    let mut largest_size = 0;
    let mut largest_links = 0;
    let mut blocks = 0;

    for (_, status) in entries {
        if app.print_owner && !app.uid_names.iter().any(|(id, _)| *id == status.uid) {
            let name = if app.convert_id_to_name {
                get_name(status.uid)
            } else {
                None
            }
            .unwrap_or_else(|| itoa::Buffer::new().format(status.uid).bytes().collect());
            longest_name_len = longest_name_len.max(name.len());
            app.uid_names.push((status.uid, name));
        }

        if app.print_group && !app.gid_names.iter().any(|(id, _)| *id == status.gid) {
            let group = if app.convert_id_to_name {
                get_group(status.gid)
            } else {
                None
            }
            .unwrap_or_else(|| itoa::Buffer::new().format(status.gid).bytes().collect());
            longest_group_len = longest_group_len.max(group.len());
            app.gid_names.push((status.gid, group));
        }

        largest_size = largest_size.max(if app.display_size_in_blocks {
            status.blocks
        } else {
            status.size
        } as usize);
        largest_links = largest_links.max(status.links as usize);
        blocks += status.blocks;
    }

    print!(app, "total ", blocks, "\n");

    let mut buf = itoa::Buffer::new();
    largest_size = buf.format(largest_size).len();
    largest_links = buf.format(largest_links).len();

    let current_year = unsafe {
        let mut localtime = core::mem::zeroed();
        let time = libc::time(core::ptr::null_mut());
        libc::localtime_r(&time, &mut localtime);
        localtime.tm_year
    };

    for direntry in entries {
        let e = &direntry.0;
        let status = &direntry.1;
        let mode = status.mode;

        let print_readable = |app: &mut App, mask| {
            if mode & mask > 0 {
                print!(app, GreenBold, "r");
            } else {
                print!(app, Gray, "-");
            }
        };

        let print_writable = |app: &mut App, mask| {
            if mode & mask > 0 {
                print!(app, YellowBold, "w");
            } else {
                print!(app, Gray, "-");
            }
        };

        let print_executable = |app: &mut App, mask| {
            print!(
                app,
                (mode & mask > 0).map((RedBold, "x")).unwrap_or((Gray, "-"))
            )
        };

        print!(
            app,
            match mode & libc::S_IFMT {
                libc::S_IFDIR => (BlueBold, "d"),
                libc::S_IFLNK => (Cyan, "l"),
                _ => (White, "."),
            }
        );

        print_readable(app, S_IRUSR);
        print_writable(app, S_IWUSR);
        print_executable(app, S_IXUSR);

        print_readable(app, S_IRGRP);
        print_writable(app, S_IWGRP);
        print_executable(app, S_IXGRP);

        print_readable(app, S_IROTH);
        print_writable(app, S_IWOTH);
        print_executable(app, S_IXOTH);

        app.out
            .push(b' ')
            .style(Style::White)
            .align_right(status.links as usize, largest_links);

        let get_or_default = |container: &[(libc::id_t, SmallVec<[u8; 24]>)], key| {
            container
                .iter()
                .find(|&it| it.0 == key)
                .map(|v| v.1.clone())
                .unwrap_or_default()
        };

        if app.print_owner {
            let name = get_or_default(&app.uid_names, status.uid);
            app.out
                .push(b' ')
                .style(YellowBold)
                .align_left(&name, longest_name_len);
        }

        if app.print_group {
            let group = get_or_default(&app.gid_names, status.gid);
            app.out
                .push(b' ')
                .style(Style::YellowBold)
                .align_left(&group, longest_group_len);
        }

        app.out.push(b' ').style(Style::GreenBold).align_right(
            if app.display_size_in_blocks {
                status.blocks
            } else {
                status.size
            } as usize,
            largest_size,
        );

        let localtime = unsafe {
            let mut localtime = core::mem::zeroed();
            libc::localtime_r(&status.time, &mut localtime);
            localtime
        };

        print!(app, " ", Style::Blue, month_abbr(localtime.tm_mon), " ");

        let day = localtime.tm_mday;
        print!(app, (day < 10).map(" "), day, " ");

        if localtime.tm_year == current_year {
            let hour = localtime.tm_hour;
            print!(app, (hour < 10).map(" "), hour, ":");

            let minute = localtime.tm_min;
            print!(app, (minute < 10).map("0"), minute);
        } else {
            print!(app, " ", localtime.tm_year + 1900);
        }

        app.out.push(b' ');

        let (style, suffix) = direntry.style(dir, app);
        app.out.style(style).write(e.name().as_bytes());
        suffix.map(|s| app.out.style(Style::White).push(s));

        if (mode & libc::S_IFMT) == libc::S_IFLNK {
            let mut buf = [0u8; 1024];
            let len = veneer::syscalls::readlinkat(dir.raw_fd(), e.name(), &mut buf).unwrap_or(0);
            if len > 0 {
                print!(app, Style::Gray, " -> ", Style::White, &buf[..len]);
            }
        }

        print!(app, Style::Reset, "\n");
    }
}

pub fn write_grid<T: DirEntry>(
    entries: &[T],
    dir: &veneer::Directory,
    app: &mut App,
    terminal_width: usize,
) {
    if entries.is_empty() {
        return;
    }

    let mut rows = entries.len();
    let mut lengths: Vec<usize> = Vec::with_capacity(entries.len());
    let mut styles = Vec::with_capacity(entries.len());
    let mut min_len: usize =
        len_utf8(entries[0].name().as_bytes()) + entries[0].style(dir, app).1.is_some() as usize;
    for e in entries {
        let style = e.style(dir, app);
        let len = len_utf8(e.name().as_bytes()) + style.1.is_some() as usize;
        lengths.push(len);
        styles.push(style);
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
            let width = column.iter().max().copied().unwrap_or(1) + 2; // 2 for padding between columns
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
            let (e, name_len, (style, suffix)) = match (
                entries.get(c * rows + r),
                lengths.get(c * rows + r),
                styles.get(c * rows + r),
            ) {
                (Some(e), Some(name_len), Some(style)) => (e, name_len, style),
                _ => continue,
            };

            app.out.style(*style).write(e.name().as_bytes());
            suffix.map(|s| app.out.style(Style::White).push(s));

            for _ in 0..(width - name_len) {
                app.out.push(b' ');
            }
        }
        app.out.style(Style::Reset).push(b'\n');
    }
}

pub fn write_stream<T: DirEntry>(entries: &[T], dir: &veneer::Directory, app: &mut App) {
    for e in entries.iter().take(entries.len() - 1) {
        let (style, suffix) = e.style(dir, app);
        app.out.style(style).write(e.name().as_bytes());
        suffix.map(|s| app.out.style(Style::White).push(s));

        app.out.style(Style::White).write(b", ");
    }
    if let Some(e) = entries.last() {
        app.out.write(e.name().as_bytes());
    }
    app.out.push(b'\n');
}

pub fn write_single_column<T: DirEntry>(entries: &[T], dir: &veneer::Directory, app: &mut App) {
    for e in entries {
        let (style, suffix) = e.style(dir, app);
        print!(app, style, e.name().as_bytes());
        suffix.map(|s| app.out.style(Style::White).push(s));
        app.out.push(b'\n');
    }
}

fn len_utf8(bytes: &[u8]) -> usize {
    core::str::from_utf8(bytes)
        .map(|s| s.graphemes(false).count())
        .unwrap_or_else(|_| bytes.len())
}

pub trait Writable {
    fn write(&self, out: &mut BufferedStdout);
}

impl Writable for &[u8] {
    fn write(&self, out: &mut BufferedStdout) {
        out.write(self);
    }
}

macro_rules! array_impl {
    ($($N:expr)+) => {
        $(
            impl Writable for &[u8; $N] {
                fn write(&self, out: &mut BufferedStdout) {
                    out.write(*self);
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
    fn write(&self, out: &mut BufferedStdout) {
        out.write(self.as_bytes());
    }
}

impl<'a> Writable for veneer::CStr<'a> {
    fn write(&self, out: &mut BufferedStdout) {
        out.write(self.as_bytes());
    }
}

impl Writable for crate::Style {
    fn write(&self, out: &mut BufferedStdout) {
        out.style(*self);
    }
}

impl Writable for u8 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for i8 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for i32 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for u64 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for i64 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl<T> Writable for Option<T>
where
    T: Writable,
{
    fn write(&self, out: &mut BufferedStdout) {
        if let Some(s) = self.as_ref() {
            s.write(out)
        }
    }
}

impl<T, U> Writable for (T, U)
where
    T: Writable,
    U: Writable,
{
    fn write(&self, out: &mut BufferedStdout) {
        self.0.write(out);
        self.1.write(out);
    }
}

pub struct BufferedStdout {
    buf: arrayvec::ArrayVec<[u8; 4096]>,
    style: Style,
    is_terminal: bool,
}

impl BufferedStdout {
    pub fn terminal() -> Self {
        Self {
            buf: arrayvec::ArrayVec::new(),
            style: Style::Reset,
            is_terminal: true,
        }
    }

    pub fn file() -> Self {
        Self {
            buf: arrayvec::ArrayVec::new(),
            style: Style::Reset,
            is_terminal: false,
        }
    }

    pub fn is_terminal(&self) -> bool {
        self.is_terminal
    }

    pub fn push(&mut self, b: u8) -> &mut Self {
        if self.buf.try_push(b).is_err() {
            write_to_stdout(&self.buf);
            self.buf.clear();
            self.buf.push(b);
        }
        self
    }

    pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
        for b in bytes {
            if self.buf.try_push(*b).is_err() {
                write_to_stdout(&self.buf);
                self.buf.clear();
                self.buf.push(*b);
            }
        }
        self
    }

    pub fn style(&mut self, style: Style) -> &mut Self {
        if self.is_terminal && self.style != style {
            self.write(style.to_bytes());
            self.style = style;
        }
        self
    }

    pub fn align_left(&mut self, value: &[u8], width: usize) -> &mut Self {
        self.write(value);
        if value.len() < width {
            for _ in 0..width - value.len() {
                self.push(b' ');
            }
        }
        self
    }

    pub fn align_right(&mut self, value: usize, width: usize) -> &mut Self {
        let mut buf = itoa::Buffer::new();
        let formatted = buf.format(value);
        if formatted.len() < width {
            for _ in 0..width - formatted.len() {
                self.push(b' ');
            }
        }
        self.write(formatted.as_bytes());
        self
    }
}

impl Drop for BufferedStdout {
    fn drop(&mut self) {
        self.style(Style::Reset);
        write_to_stdout(&self.buf);
    }
}

fn write_to_stdout(bytes: &[u8]) {
    let mut bytes_written = 0;
    while bytes_written < bytes.len() {
        bytes_written += syscalls::write(libc::STDOUT_FILENO, &bytes[bytes_written..]).unwrap();
    }
}

fn month_abbr(month: libc::c_int) -> &'static [u8] {
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

trait BoolExt {
    fn map<T>(self, value: T) -> Option<T>;
}

impl BoolExt for bool {
    fn map<T>(self, value: T) -> Option<T> {
        if self {
            Some(value)
        } else {
            None
        }
    }
}
