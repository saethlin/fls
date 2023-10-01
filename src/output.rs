use crate::{
    cli::App,
    directory::{DirEntry, DirEntryExt},
    utils::Buffer,
    Status, Style,
};
use alloc::vec::Vec;
use veneer::{fs::Directory, syscalls, CStr};

use libc::{S_IRGRP, S_IROTH, S_IRUSR, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP, S_IXOTH, S_IXUSR};
use unicode_width::UnicodeWidthStr;

#[macro_export]
macro_rules! print {
    ($app:expr, $($item:expr),+) => {
        {
        use $crate::output::Writable;
        $($item.write(&mut $app.out);)*
    }};
}

fn print_rwx(app: &mut App, mode: u32, read_mask: u32, write_mask: u32, execute_mask: u32) {
    use Style::*;

    for (mask, color, chr) in [
        (read_mask, YellowBold, b'r'),
        (write_mask, RedBold, b'w'),
        (execute_mask, GreenBold, b'x'),
    ] {
        if mode & mask > 0 {
            app.out.style(color).push(chr);
        } else {
            app.out.style(Gray).push(b'-');
        }
    }
}

pub fn write_details(entries: &[(DirEntry, Option<Status>)], dir: &Directory, app: &mut App) {
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
            longest_name_len = longest_name_len.max(app.getpwuid(status.uid).len());
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

    let mut buf = Buffer::new();
    largest_size = buf.format(largest_size as u64).len();
    largest_links = buf.format(largest_links as u64).len();
    inode_len = buf.format(inode_len as u64).len();
    blocks_len = buf.format(blocks_len as u64).len();

    let current_time = syscalls::gettimeofday().unwrap().tv_sec;
    let one_year = 365 * 24 * 60 * 60;

    for direntry in entries {
        let e = &direntry.0;
        let status = direntry.1.clone().unwrap_or_default();
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
            .style(White)
            .align_right(status.links, largest_links);

        if app.print_owner {
            let name = app.getpwuid(status.uid);
            app.out
                .push(b' ')
                .style(YellowBold)
                .align_left(name, longest_name_len);
        }

        if app.print_group {
            let group = app.getgrgid(status.gid);
            app.out
                .push(b' ')
                .style(YellowBold)
                .align_left(group, longest_group_len);
        }

        app.out
            .push(b' ')
            .style(GreenBold)
            .align_right(status.size as u64, largest_size);

        let localtime = app.convert_to_localtime(status.time);

        print!(app, " ", Blue, month_abbr(localtime.month), " ");

        let day = localtime.day_of_month;
        print!(app, (day < 10).map(" "), day, " ");

        if current_time - status.time < one_year / 2 {
            let hour = localtime.hour;
            print!(app, (hour < 10).map("0"), hour, ":");

            let minute = localtime.minute;
            print!(app, (minute < 10).map("0"), minute);
        } else {
            print!(app, " ", localtime.year + 1900);
        }

        app.out.push(b' ');

        let (mut style, suffix) = direntry.style(dir, app);
        // FIXME: This is a hack to get red-colored broken symlinks in -l output.
        // This logic is at completely the wrong place, and it's setting the style to RedBold, not
        // BrokenLink.
        if (mode & libc::S_IFMT) == libc::S_IFLNK
            && app.color == crate::cli::Color::Always
            && syscalls::faccessat(dir.raw_fd(), e.name, libc::F_OK).is_err()
        {
            style = RedBold;
        }
        print!(app, style, e.name, suffix.map(|s| (White, s)));

        if (mode & libc::S_IFMT) == libc::S_IFLNK {
            let mut buf = [0u8; 1024];
            if let Ok(linked_to) = syscalls::readlinkat(dir.raw_fd(), e.name, &mut buf) {
                print!(app, Gray, " -> ", White, linked_to);
            }
        }

        print!(app, Reset, "\n");
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
            .sum::<i64>() as u64,
        "\n"
    );
}

pub struct LayoutCursor {
    column: usize,
    left_in_this_column: usize,
    rows: usize,
    this_layout_width: usize,
}

pub fn write_grid(
    entries: &[(DirEntry, Option<Status>)],
    dir: &Directory,
    app: &mut App,
    terminal_width: usize,
) {
    use Style::*;

    if app.display_size_in_blocks {
        print_total_blocks(entries, app);
    }

    if entries.is_empty() {
        return;
    }

    let inode_len = if app.print_inode {
        let inode = entries.iter().map(|e| e.inode()).max().unwrap_or(0);
        Buffer::new().format(inode).len()
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
        Buffer::new().format(blocks as u64).len()
    } else {
        0
    };

    // We want to determine the maximum number of columns we can use to lay out these entries.
    // So we simulate arranging the entries in every possible layout at the same time. Notionally,
    // we keep a Vec of column widths (widest name in each column) for every number of columns, and
    // when we add an entry to a column which makes the sum of all columns for that layout too
    // large, we discard it.

    let sum_to = |a| a * (a + 1) / 2;

    let mut lengths = Vec::with_capacity(entries.len());
    let mut styles = Vec::with_capacity(entries.len());

    let max_possible_columns = core::cmp::min(terminal_width / 3, entries.len());

    let mut layouts = Vec::with_capacity(sum_to(max_possible_columns) - 1);
    let mut cursors = Vec::with_capacity(max_possible_columns - 1);

    for i in 2..=max_possible_columns {
        layouts.extend(core::iter::repeat(0).take(i));
        // current position, increments left until we move to the next column
        let rows = (entries.len() + i - 1) / i;
        cursors.push(LayoutCursor {
            column: 0,
            left_in_this_column: rows,
            rows,
            this_layout_width: (i - 1) * 2, // Initially, just the 2 spaces between each column
        });
        if rows == 1 {
            break;
        }
    }

    for entry in entries {
        let style = entry.style(dir, app);
        let len =
            len_utf8(entry.name().as_bytes()) + style.1.is_some() as usize + inode_len + blocks_len;
        lengths.push(len);
        styles.push(style);

        for i in (0..cursors.len()).rev() {
            let layout_start = sum_to(i + 1) - 1;

            let current = &mut layouts[layout_start + cursors[i].column];
            if len > *current {
                cursors[i].this_layout_width += len - *current;
                *current = len;
            }

            if cursors[i].this_layout_width > terminal_width {
                cursors.pop();
            } else {
                cursors[i].left_in_this_column -= 1;
                if cursors[i].left_in_this_column == 0 {
                    cursors[i].column += 1;
                    cursors[i].left_in_this_column = cursors[i].rows;
                }
            }
        }
    }

    let rows = cursors.last().map(|c| c.rows).unwrap_or(entries.len());

    let mut widths = Vec::new();
    widths.extend(
        lengths
            .chunks(rows)
            .map(|column| column.iter().max().copied().unwrap_or(1) + 2),
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
                    .style(Magenta)
                    .align_right(e.inode(), inode_len)
                    .push(b' ');
            }

            if app.display_size_in_blocks {
                app.out
                    .style(White)
                    .align_right(e.blocks(), blocks_len)
                    .push(b' ');
            }

            app.out.style(*style);
            print!(app, e.name(), suffix.map(|s| (White, s)));

            for _ in 0..(width - name_len) {
                app.out.push(b' ');
            }
        }
        app.out.style(Reset).push(b'\n');
    }

    app.out.flush();
}

pub fn write_stream(entries: &[(DirEntry, Option<Status>)], dir: &Directory, app: &mut App) {
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

pub fn write_single_column(entries: &[(DirEntry, Option<Status>)], dir: &Directory, app: &mut App) {
    if app.display_size_in_blocks {
        print_total_blocks(entries, app);
    }

    let inode_len = if app.print_inode {
        let inode = entries.iter().map(DirEntryExt::inode).max().unwrap_or(0);
        Buffer::new().format(inode).len()
    } else {
        0
    };

    let blocks_len = if app.display_size_in_blocks {
        let blocks = entries.iter().map(DirEntryExt::blocks).max().unwrap_or(0);
        Buffer::new().format(blocks).len()
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

#[inline(never)]
fn len_utf8(bytes: &[u8]) -> usize {
    if bytes.iter().all(u8::is_ascii) {
        bytes.len()
    } else {
        core::str::from_utf8(bytes)
            .map(|s| s.width())
            .unwrap_or(bytes.len())
    }
}

pub trait Writable {
    fn write(&self, out: &mut OutputBuffer);
}

impl Writable for &[u8] {
    fn write(&self, out: &mut OutputBuffer) {
        out.write(self);
    }
}

impl<const N: usize> Writable for &[u8; N] {
    fn write(&self, out: &mut OutputBuffer) {
        #[allow(clippy::explicit_auto_deref)] // Invalid suggestion
        out.write(*self);
    }
}

impl Writable for &str {
    fn write(&self, out: &mut OutputBuffer) {
        out.write(self.as_bytes());
    }
}

impl<'a> Writable for CStr<'a> {
    fn write(&self, out: &mut OutputBuffer) {
        out.write(self.as_bytes());
    }
}

impl Writable for Style {
    fn write(&self, out: &mut OutputBuffer) {
        out.style(*self);
    }
}

impl Writable for u8 {
    fn write(&self, out: &mut OutputBuffer) {
        out.push(*self);
    }
}

impl Writable for i64 {
    fn write(&self, out: &mut OutputBuffer) {
        use core::convert::TryFrom;
        let mut buf = Buffer::new();
        out.write(buf.format(u64::try_from(*self).unwrap()));
    }
}

impl Writable for i32 {
    fn write(&self, out: &mut OutputBuffer) {
        (*self as i64).write(out);
    }
}

impl Writable for u32 {
    fn write(&self, out: &mut OutputBuffer) {
        (*self as u64).write(out);
    }
}

impl Writable for u64 {
    fn write(&self, out: &mut OutputBuffer) {
        let mut buf = Buffer::new();
        out.write(buf.format(*self));
    }
}

impl Writable for usize {
    fn write(&self, out: &mut OutputBuffer) {
        (*self as u64).write(out);
    }
}

impl<T, U> Writable for (T, U)
where
    T: Writable,
    U: Writable,
{
    fn write(&self, out: &mut OutputBuffer) {
        self.0.write(out);
        self.1.write(out);
    }
}

impl<T> Writable for Option<T>
where
    T: Writable,
{
    fn write(&self, out: &mut OutputBuffer) {
        if let Some(s) = self.as_ref() {
            s.write(out);
        }
    }
}

pub struct OutputBuffer {
    buf: [u8; 4096],
    buf_used: usize,
    style: Style,
    fd: i32,
    pub color: bool,
}

impl OutputBuffer {
    pub fn to_fd(fd: libc::c_int) -> Self {
        Self {
            buf: [0u8; 4096],
            buf_used: 0,
            style: Style::Reset,
            color: true,
            fd,
        }
    }

    pub fn push(&mut self, b: u8) -> &mut Self {
        if let Some(out) = self.buf.get_mut(self.buf_used) {
            *out = b;
        } else {
            self.flush();
            self.buf[0] = b;
        }
        self.buf_used += 1;
        self
    }

    #[inline(never)]
    pub fn flush(&mut self) {
        write_all(&self.buf[..self.buf_used], self.fd);
        self.buf_used = 0;
    }

    pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
        if bytes.len() + self.buf_used >= self.buf.len() {
            self.flush();
        }
        if bytes.len() > self.buf.len() {
            write_all(bytes, self.fd);
        } else {
            let end = self.buf_used + bytes.len();
            self.buf[self.buf_used..end].copy_from_slice(bytes);
            self.buf_used += bytes.len();
        }
        self
    }

    pub fn style(&mut self, style: Style) -> &mut Self {
        if !self.color {
            return self;
        };
        if self.style != style {
            style.write_to(self);
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
        let mut buf = Buffer::new();
        let formatted = buf.format(value);
        if formatted.len() < width {
            for _ in 0..width - formatted.len() {
                self.push(b' ');
            }
        }
        self.write(formatted);
        self
    }
}

impl core::fmt::Write for OutputBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

impl Drop for OutputBuffer {
    fn drop(&mut self) {
        self.style(Style::Reset);
        // Panicking in a Drop is probably a bad idea, so we prefer to be slightly wrong
        // in the case that our index has become invalid
        if let Some(buf) = self.buf.get(..self.buf_used) {
            write_all(buf, self.fd);
        }
    }
}

#[inline(never)]
fn write_all(bytes: &[u8], fd: i32) {
    let mut bytes_written = 0;
    while bytes_written < bytes.len() {
        bytes_written += syscalls::write(fd, &bytes[bytes_written..]).unwrap_or_else(|_| {
            syscalls::exit(-1);
        });
    }
}

static MONTH_NAMES: &[&[u8]] = &[
    b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
];

fn month_abbr(month: libc::c_int) -> &'static [u8] {
    MONTH_NAMES.get(month as usize).copied().unwrap_or(b"???")
}

// This code was translated almost directly from the implementation in GNU ls
//
pub fn vercmp(s1_cstr: CStr, s2_cstr: CStr) -> core::cmp::Ordering {
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
