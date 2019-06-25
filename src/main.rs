#![feature(lang_items)]
#![feature(start, alloc_error_handler)]
#![no_std]
#![no_main]

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use arrayvec::ArrayVec;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[lang = "eh_unwind_resume"]
#[no_mangle]
pub extern "C" fn rust_eh_unwind_resume() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    loop {}
}

mod error;

use error::Error;
use smallvec::SmallVec;

struct ReadDir {
    dir: *mut libc::DIR,
}

impl ReadDir {
    fn new(path: &mut ArrayVec<[u8; libc::PATH_MAX as usize]>) -> Result<Self, Error> {
        if path.last() != Some(&0) {
            path.push(0);
        }
        unsafe {
            let dir = libc::opendir(path.as_ptr() as *const i8);
            if dir.is_null() {
                Err(Error::last_os_error())
            } else {
                Ok(Self { dir })
            }
        }
    }
}

#[derive(Clone, Copy)]
enum DType {
    Fifo = 1,
    Character = 2,
    Directory = 4,
    Block = 6,
    Regular = 8,
    Symlink = 10,
    Socket = 12,
}

struct RawDirEntry {
    name: SmallVec<[u8; 24]>,
    d_type: Option<DType>,
}

impl Iterator for ReadDir {
    type Item = RawDirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ptr = libc::readdir(self.dir);
            if ptr.is_null() {
                None
            } else {
                let mut name = SmallVec::new();
                for c in (*ptr).d_name.iter() {
                    if *c == 0 {
                        break;
                    }
                    name.push(*c as u8);
                }
                let d_type = match (*ptr).d_type {
                    libc::DT_UNKNOWN => None,
                    libc::DT_FIFO => Some(DType::Fifo),
                    libc::DT_CHR => Some(DType::Character),
                    libc::DT_DIR => Some(DType::Directory),
                    libc::DT_BLK => Some(DType::Block),
                    libc::DT_REG => Some(DType::Regular),
                    libc::DT_LNK => Some(DType::Symlink),
                    libc::DT_SOCK => Some(DType::Socket),
                    _ => None,
                };
                Some(RawDirEntry { name, d_type })
            }
        }
    }
}

impl RawDirEntry {
    fn name(&self) -> &[u8] {
        &self.name
    }

    fn style(
        &self,
        root: &mut ArrayVec<[u8; libc::PATH_MAX as usize]>,
    ) -> Result<(Color, Style), Error> {
        use Color::*;
        use Style::*;
        while root.last() == Some(&0) {
            root.pop();
        }
        match self.d_type {
            Some(DType::Directory) => Ok((Blue, Bold)),
            Some(DType::Symlink) => Ok((Cyan, Bold)),
            Some(DType::Regular) => unsafe {
                if root.last() != Some(&b'/') {
                    root.push(b'/');
                }
                for b in &self.name {
                    root.try_push(*b)?;
                }
                root.try_push(0)?;
                let executable = libc::access(root.as_ptr() as *const i8, libc::X_OK);
                while root.last() != Some(&b'/') {
                    root.pop();
                }
                if executable == 0 {
                    // the file is executable
                    Ok((Green, Bold))
                } else {
                    if *libc::__errno_location() == libc::EACCES {
                        // We don't have write permissions, this is fine
                        Ok((White, Regular))
                    } else {
                        // Something went wrong
                        Err(Error::last_os_error())
                    }
                }
            },
            _ => Ok((White, Regular)),
        }
    }
}

// TODO: Reducing the number of escape sequences that's printed is worth ~10% perf
// But right now there seems to be an alacritty bug with resetting colors after a newline
fn write_grid(
    root: &mut ArrayVec<[u8; libc::PATH_MAX as usize]>,
    out: &mut BufferedStdout,
    terminal_width: usize,
) -> Result<(), Error> {
    let mut entries = Vec::new();
    for e in ReadDir::new(root)?.filter(|e| e.name().get(0) != Some(&b'.')) {
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

            let (color, style) = e.style(root)?;
            out.write_all(color.as_ref())?;
            out.write_all(style.as_ref())?;
            out.write_all(e.name())?;
            out.write_all(Style::Regular.as_ref())?;
            //out.write_all(&termion::style::Reset.as_ref())?;

            for _ in 0..width - e.name().len() {
                out.write_all(b" ")?;
            }
        }
        out.write_all(b"\n")?;
    }

    Ok(())
}

fn write_single_column(
    root: &mut ArrayVec<[u8; libc::PATH_MAX as usize]>,
    out: &mut BufferedStdout,
) -> Result<(), Error> {
    let mut entries = Vec::new();
    for e in ReadDir::new(root)?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    for e in entries {
        out.write_all(e.name())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

#[no_mangle] // ensure that this symbol is called `main` in the output
pub extern "C" fn main(argc: i32, argv: *const *const u8) -> i32 {
    match run(argc, argv) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

fn run(argc: i32, argv: *const *const u8) -> Result<(), Error> {
    let mut out = BufferedStdout {
        buf: Default::default(),
    };

    let terminal_width = unsafe {
        let mut winsize = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if libc::ioctl(1, libc::TIOCGWINSZ, &mut winsize as *mut libc::winsize) == -1 {
            Err(Error::last_os_error())
        } else {
            Ok(winsize.ws_col as usize)
        }
    };

    let mut root: ArrayVec<[u8; libc::PATH_MAX as usize]> = ArrayVec::new();
    if argc < 2 {
        root.push(b'.');
        if let Ok(width) = terminal_width {
            write_grid(&mut root, &mut out, width)?;
        } else {
            write_single_column(&mut root, &mut out)?;
        }
    } else {
        for a in 1..argc {
            root.clear();
            unsafe {
                let mut arg: *const u8 = *argv.offset(a as isize);
                loop {
                    let b = *arg;
                    root.push(b);
                    if b == 0 {
                        break;
                    }
                    arg = arg.offset(1);
                }
            }
            if let Ok(width) = terminal_width {
                write_grid(&mut root, &mut out, width)?;
            } else {
                write_single_column(&mut root, &mut out)?;
            }
        }
    }

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    //Red,
    Green,
    //Yellow,
    Blue,
    //Magenta,
    Cyan,
    White,
}

impl AsRef<[u8]> for Color {
    fn as_ref(&self) -> &'static [u8] {
        match self {
            //Color::Red => b"\x1B[31m",
            Color::Green => b"\x1B[32m",
            //Color::Yellow => b"\x1B[33m",
            Color::Blue => b"\x1B[34m",
            //Color::Magenta => b"\x1B[35m",
            Color::Cyan => b"\x1B[36m",
            Color::White => b"\x1B[37m",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Regular,
    Bold,
}

impl AsRef<[u8]> for Style {
    fn as_ref(&self) -> &'static [u8] {
        match self {
            Style::Regular => b"\x1B[m",
            Style::Bold => b"\x1B[1m",
        }
    }
}

struct BufferedStdout {
    buf: arrayvec::ArrayVec<[u8; 1024]>,
}

impl BufferedStdout {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        for b in bytes {
            if let Err(_) = self.buf.try_push(*b) {
                // write everything in the buffer to stdout
                unsafe {
                    let mut bytes_written = 0;
                    while bytes_written < self.buf.len() {
                        let ret = libc::write(
                            libc::STDOUT_FILENO,
                            (&self.buf[bytes_written..]).as_ptr() as *const libc::c_void,
                            self.buf.len() - bytes_written as usize,
                        );
                        if ret == -1 {
                            return Err(Error::last_os_error());
                        } else {
                            bytes_written += ret as usize;
                        }
                    }
                }
                self.buf.clear();
                unsafe {
                    self.buf.push_unchecked(*b);
                }
            }
        }
        Ok(())
    }
}

impl Drop for BufferedStdout {
    fn drop(&mut self) {
        unsafe {
            let mut bytes_written = 0;
            while bytes_written < self.buf.len() {
                bytes_written += libc::write(
                    libc::STDOUT_FILENO,
                    (&self.buf[bytes_written..]).as_ptr() as *const libc::c_void,
                    self.buf.len() - bytes_written as usize,
                ) as usize;
            }
        }
    }
}
