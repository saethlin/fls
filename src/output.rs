use crate::directory::DirEntryExt;
use crate::veneer;
use crate::veneer::syscalls;
use crate::veneer::DirEntry;
use crate::{cli::App, Status, Style};
use bumpalo::collections::Vec;

use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};
use unicode_segmentation::UnicodeSegmentation;

#[macro_export]
macro_rules! print {
    ($app:expr, $($item:expr),+) => {
        {
        use crate::output::Writable;
        $($item.write(&mut $app.out);)*
    }};
}

macro_rules! error {
    ($($item:expr),+) => {
        {
        let mut err = alloc::vec::Vec::new();
        err.extend_from_slice(b"fls: ");
        $(err.extend($item);)*
        if err.last() != Some(&b'\n') {
            err.push(b'\n');
        }
        let _ = crate::veneer::syscalls::write(2, &err[..]);
    }};
}

#[inline(never)]
fn print_rwx(app: &mut App, mode: u32, read_mask: u32, write_mask: u32, execute_mask: u32) {
    use crate::Style::*;

    if mode & read_mask > 0 {
        app.out.style(YellowBold).push(b'r');
    } else {
        app.out.style(Gray).push(b'-');
    }

    if mode & write_mask > 0 {
        app.out.style(RedBold).push(b'w');
    } else {
        app.out.style(Gray).push(b'-');
    }

    if mode & execute_mask > 0 {
        app.out.style(GreenBold).push(b'x');
    } else {
        app.out.style(Gray).push(b'-');
    }
}

#[inline(never)]
pub fn write_details(
    entries: &[(DirEntry, Option<Status>)],
    dir: &veneer::Directory,
    app: &mut App,
) {
    use Style::*;

    let mut longest_name_len = 1;
    let mut longest_group_len = 1;
    let mut largest_size = 0;
    let mut largest_links = 0;
    let mut blocks = 0;
    let mut inode_len = 0;
    let mut blocks_len = 0;

    for status in entries.iter().filter_map(|e| e.1.as_ref()) {
        if app.print_owner {
            longest_name_len = longest_name_len.max(app.getpwuid(status.uid).len())
        }

        if app.print_group {
            longest_group_len = longest_group_len.max(app.getgrgid(status.gid).len());
        }

        largest_size = largest_size.max(status.size as usize);
        largest_links = largest_links.max(status.links as usize);
        inode_len = inode_len.max(status.inode as usize);
        blocks_len = blocks_len.max(status.blocks as usize);
        blocks += status.blocks * status.block_size / 8192;
    }

    print!(app, "total ", blocks, "\n");

    largest_size = largest_size.log10();
    largest_links = largest_links.log10();
    inode_len = inode_len.log10();
    blocks_len = blocks_len.log10();

    let current_time = unsafe { libc::time(core::ptr::null_mut()) };
    let one_year = 365 * 24 * 60 * 60;

    for direntry in entries {
        let e = &direntry.0;
        let status = direntry.1.clone().unwrap_or(Status::default());
        let mode = status.mode;

        if app.print_inode {
            app.out
                .style(Magenta)
                .align_right(status.inode, inode_len)
                .push(b' ');
        }

        if app.display_size_in_blocks {
            app.out
                .style(White)
                .align_right(status.blocks as u64, blocks_len)
                .push(b' ');
        }

        print!(
            app,
            match mode & libc::S_IFMT {
                libc::S_IFDIR => (BlueBold, "d"),
                libc::S_IFLNK => (Cyan, "l"),
                _ => (White, "-"),
            }
        );

        print_rwx(app, mode, S_IRUSR, S_IWUSR, S_IXUSR);
        print_rwx(app, mode, S_IRGRP, S_IWGRP, S_IXGRP);
        print_rwx(app, mode, S_IROTH, S_IWOTH, S_IXOTH);

        app.out
            .push(b' ')
            .style(Style::White)
            .align_right(status.links, largest_links);

        if app.print_owner {
            let name = app.getpwuid(status.uid);
            app.out
                .push(b' ')
                .style(YellowBold)
                .align_left(&name, longest_name_len);
        }

        if app.print_group {
            let group = app.getgrgid(status.gid);
            app.out
                .push(b' ')
                .style(Style::YellowBold)
                .align_left(&group, longest_group_len);
        }

        app.out
            .push(b' ')
            .style(Style::GreenBold)
            .align_right(status.size as u64, largest_size);

        let localtime = unsafe {
            let mut localtime = core::mem::zeroed();
            libc::localtime_r(&status.time, &mut localtime);
            localtime
        };

        print!(app, " ", Style::Blue, month_abbr(localtime.tm_mon), " ");

        let day = localtime.tm_mday;
        print!(app, (day < 10).map(" "), day, " ");

        if current_time - status.time < one_year / 2 {
            let hour = localtime.tm_hour;
            print!(app, (hour < 10).map("0"), hour, ":");

            let minute = localtime.tm_min;
            print!(app, (minute < 10).map("0"), minute);
        } else {
            print!(app, " ", localtime.tm_year + 1900);
        }

        app.out.push(b' ');

        let (mut style, suffix) = direntry.style(dir, app);
        // FIXME: This is a hack to get red-colored broken symlinks in -l output.
        // This logic is at completely the wrong place, and it's setting the style to RedBold, not
        // BrokenLink.
        if (mode & libc::S_IFMT) == libc::S_IFLNK
            && app.color == crate::cli::Color::Always
            && veneer::syscalls::faccessat(dir.raw_fd(), e.name, libc::F_OK).is_err()
        {
            style = Style::RedBold;
        }
        print!(app, style, e.name, suffix.map(|s| (Style::White, s)));

        if (mode & libc::S_IFMT) == libc::S_IFLNK {
            let mut buf = [0u8; 1024];
            if let Ok(linked_to) = veneer::syscalls::readlinkat(dir.raw_fd(), e.name, &mut buf) {
                print!(app, Style::Gray, " -> ", Style::White, linked_to);
            }
        }

        print!(app, Style::Reset, "\n");
    }
}

fn print_total_blocks(entries: &[(DirEntry, Option<Status>)], app: &mut App) {
    print!(
        app,
        "total ",
        entries
            .iter()
            .filter_map(|(_, s)| s.as_ref())
            .map(|status| status.blocks)
            .sum::<i64>(),
        "\n"
    );
}

#[inline(never)]
pub fn write_grid(
    entries: &[(DirEntry, Option<Status>)],
    dir: &veneer::Directory,
    app: &mut App,
    terminal_width: usize,
) {
    if app.display_size_in_blocks {
        print_total_blocks(entries, app);
    }

    if entries.is_empty() {
        return;
    }

    let inode_len = if app.print_inode {
        let inode = entries.iter().map(|e| e.inode()).max().unwrap_or(0);
        itoa::Buffer::new().format(inode as u64).len()
    } else {
        0
    };

    let blocks_len = if app.display_size_in_blocks {
        let blocks = entries
            .iter()
            .filter_map(|(_, s)| s.as_ref())
            .map(|status| status.blocks)
            .max()
            .unwrap_or(0);
        itoa::Buffer::new().format(blocks as u64).len()
    } else {
        0
    };

    let bump = bumpalo::Bump::new();

    let mut lengths: Vec<usize> = Vec::with_capacity_in(entries.len(), &bump);
    let mut styles = Vec::with_capacity_in(entries.len(), &bump);

    let max_possible_columns = terminal_width / 3;

    let mut layouts = bumpalo::collections::Vec::new_in(&bump);
    let mut cursors = bumpalo::collections::Vec::new_in(&bump);

    for i in 2..=max_possible_columns {
        layouts.push(bumpalo::collections::Vec::from_iter_in(
            core::iter::repeat(0).take(i),
            &bump,
        ));
        // current position, increments left until we move to the next column
        let rows = (entries.len() + i - 1) / i;
        cursors.push((0, rows, rows));
        if rows == 1 {
            break;
        }
    }

    /*
    struct GridCursor {
        column: usize,
        left_in_this_column: usize,
        rows: usize,
    }
    */

    for entry in entries {
        let style = entry.style(dir, app);
        let len =
            len_utf8(entry.name().as_bytes()) + style.1.is_some() as usize + inode_len + blocks_len;
        lengths.push(len);
        styles.push(style);

        for i in (0..cursors.len()).rev() {
            let current = &mut layouts[i][cursors[i].0];
            *current = (*current).max(len);
            if layouts[i].iter().copied().sum::<usize>() + 2 * (layouts[i].len() - 1)
                > terminal_width
            {
                cursors.pop();
                layouts.pop();
            } else {
                cursors[i].1 -= 1;
                if cursors[i].1 == 0 {
                    cursors[i].0 += 1;
                    cursors[i].1 = cursors[i].2;
                }
            }
        }
    }

    let rows = cursors.last().map(|c| c.2).unwrap_or(entries.len());

    let mut widths = Vec::from_iter_in(
        lengths
            .chunks(rows)
            .map(|column| column.iter().max().copied().unwrap_or(1) + 2),
        &bump,
    );
    if let Some(width) = widths.last_mut() {
        *width -= 2;
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

            if app.print_inode {
                app.out
                    .style(Style::Magenta)
                    .align_right(e.inode(), inode_len)
                    .push(b' ');
            }

            if app.display_size_in_blocks {
                app.out
                    .style(Style::White)
                    .align_right(e.blocks(), blocks_len)
                    .push(b' ');
            }

            app.out.style(*style);
            print!(app, e.name(), suffix.map(|s| (Style::White, s)));

            for _ in 0..(width - name_len) {
                app.out.push(b' ');
            }
        }
        app.out.style(Style::Reset).push(b'\n');
    }
}

#[inline(never)]
pub fn write_stream(
    entries: &[(DirEntry, Option<Status>)],
    dir: &veneer::Directory,
    app: &mut App,
) {
    if app.display_size_in_blocks {
        print_total_blocks(entries, app);
    }

    for e in entries.iter().take(entries.len() - 1) {
        if app.print_inode {
            print!(app, Style::Magenta, e.inode(), " ");
        }

        if app.display_size_in_blocks {
            print!(app, Style::White, e.blocks(), " ");
        }

        let (style, suffix) = e.style(dir, app);
        print!(
            app,
            style,
            e.name(),
            suffix.map(|s| (Style::White, s)),
            Style::White,
            ", "
        );
    }
    if let Some(e) = entries.last() {
        app.out.write(e.name().as_bytes());
    }
    app.out.push(b'\n');
}

#[inline(never)]
pub fn write_single_column(
    entries: &[(DirEntry, Option<Status>)],
    dir: &veneer::Directory,
    app: &mut App,
) {
    if app.display_size_in_blocks {
        print_total_blocks(entries, app);
    }

    let inode_len = if app.print_inode {
        let inode = entries.iter().map(DirEntryExt::inode).max().unwrap_or(0);
        itoa::Buffer::new().format(inode).len()
    } else {
        0
    };

    let blocks_len = if app.display_size_in_blocks {
        let blocks = entries.iter().map(DirEntryExt::blocks).max().unwrap_or(0);
        itoa::Buffer::new().format(blocks).len()
    } else {
        0
    };

    for e in entries {
        if app.print_inode {
            app.out
                .style(Style::Magenta)
                .align_right(e.inode(), inode_len)
                .push(b' ');
        }

        if app.display_size_in_blocks {
            app.out
                .style(Style::White)
                .align_right(e.blocks(), blocks_len)
                .push(b' ');
        }

        let (style, suffix) = e.style(dir, app);
        print!(
            app,
            style,
            e.name(),
            suffix.map(|s| (Style::White, s)),
            Style::Reset,
            "\n"
        );
    }
}

fn len_utf8(bytes: &[u8]) -> usize {
    if bytes.iter().all(u8::is_ascii) {
        bytes.len()
    } else {
        core::str::from_utf8(bytes)
            .map(|s| s.graphemes(true).count())
            .unwrap_or_else(|_| bytes.len())
    }
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
        out.push(*self);
    }
}

impl Writable for i64 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for i32 {
    fn write(&self, out: &mut BufferedStdout) {
        (*self as i64).write(out)
    }
}

impl Writable for u64 {
    fn write(&self, out: &mut BufferedStdout) {
        let mut buf = itoa::Buffer::new();
        out.write(buf.format(*self).as_bytes());
    }
}

impl Writable for usize {
    fn write(&self, out: &mut BufferedStdout) {
        (*self as u64).write(out)
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

impl<T> Writable for Option<T>
where
    T: Writable,
{
    fn write(&self, out: &mut BufferedStdout) {
        if let Some(s) = self.as_ref() {
            s.write(out);
        }
    }
}

pub struct BufferedStdout {
    buf: [u8; 4096],
    buf_used: usize,
    style: Style,
    is_terminal: bool,
}

impl BufferedStdout {
    pub fn terminal() -> Self {
        Self {
            buf: [0u8; 4096],
            buf_used: 0,
            style: Style::Reset,
            is_terminal: true,
        }
    }

    pub fn file() -> Self {
        Self {
            buf: [0u8; 4096],
            buf_used: 0,
            style: Style::Reset,
            is_terminal: false,
        }
    }

    pub fn push(&mut self, b: u8) -> &mut Self {
        if let Some(out) = self.buf.get_mut(self.buf_used) {
            *out = b;
        } else {
            self.flush_to_stdout();
            self.buf[0] = b;
        }
        self.buf_used += 1;
        self
    }

    #[cold]
    #[inline(never)]
    pub fn flush_to_stdout(&mut self) {
        write_to_stdout(&self.buf[..self.buf_used]);
        self.buf_used = 0;
    }

    pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
        if bytes.len() + self.buf_used >= self.buf.len() {
            self.flush_to_stdout();
        }
        let end = self.buf_used + bytes.len();
        self.buf[self.buf_used..end].copy_from_slice(bytes);
        self.buf_used += bytes.len();
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

    pub fn align_right(&mut self, value: u64, width: usize) -> &mut Self {
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
        // Panicking in a Drop is probably a bad idea, so we prefer to be slightly wrong
        // in the case that our index has become invalid
        if let Some(buf) = self.buf.get(..self.buf_used) {
            write_to_stdout(buf);
        }
    }
}

fn write_to_stdout(bytes: &[u8]) {
    let mut bytes_written = 0;
    while bytes_written < bytes.len() {
        bytes_written += syscalls::write(libc::STDOUT_FILENO, &bytes[bytes_written..])
            .unwrap_or_else(|_| {
                syscalls::exit(-1);
                0
            });
    }
}

fn month_abbr(month: libc::c_int) -> &'static [u8] {
    let month_names = [
        b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov",
        b"Dec",
    ];
    month_names.get(month as usize).copied().unwrap_or(b"???")
}

// This code was translated almost directly from the implementation in GNU ls
//
pub fn vercmp(s1_cstr: veneer::CStr, s2_cstr: veneer::CStr) -> core::cmp::Ordering {
    use core::cmp::Ordering;
    let s1 = s1_cstr.as_bytes();
    let s2 = s2_cstr.as_bytes();
    let mut s1_pos: usize = 0;
    let mut s2_pos: usize = 0;

    while s1_pos < s1.len() || s2_pos < s2.len() {
        let mut first_diff = Ordering::Equal;
        // Compare lexicographic until we hit a numeral
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
        // Skip leading zeroes in both strings
        while s1.get(s1_pos) == Some(&b'0') {
            s1_pos += 1;
        }
        while s2.get(s2_pos) == Some(&b'0') {
            s2_pos += 1;
        }
        // Advance forward while they are both characters
        while s1.digit_at(s1_pos) && s2.digit_at(s2_pos) {
            if first_diff == Ordering::Equal {
                first_diff = s1.get(s1_pos).cmp(&s2.get(s2_pos));
            }
            s1_pos += 1;
            s2_pos += 1;
        }
        // If one string has more digits than the other, the number is larger
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
