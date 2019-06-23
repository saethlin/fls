use std::path::Path;

struct ReadDir<'a> {
    dir: *mut libc::DIR,
    marker: std::marker::PhantomData<&'a libc::DIR>,
}

impl<'a> ReadDir<'a> {
    fn new<P: AsRef<Path>>(path: P) -> Option<Self> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let path = CString::new(path.as_ref().as_os_str().as_bytes()).ok()?;
        unsafe {
            let dir = libc::opendir(path.as_ptr());
            if dir.is_null() {
                None
            } else {
                Some(Self {
                    dir,
                    marker: Default::default(),
                })
            }
        }
    }
}

#[derive(Debug)]
enum DType {
    Fifo = 1,
    Character = 2,
    Directory = 4,
    Block = 6,
    Regular = 8,
    Symlink = 10,
    Socket = 12,
}

#[derive(Debug)]
struct RawDirEntry<'a> {
    name: &'a CStr,
    d_type: Option<DType>,
}

use std::ffi::CStr;
impl<'a> Iterator for ReadDir<'a> {
    type Item = RawDirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let ptr = libc::readdir(self.dir);
            if ptr.is_null() {
                None
            } else {
                let name = CStr::from_ptr(&(*ptr).d_name as *const i8);
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

impl<'a> RawDirEntry<'a> {
    fn style(&self, root: &mut Vec<u8>) -> (Color, Style) {
        use Color::*;
        use Style::*;
        match self.d_type.as_ref().unwrap() {
            DType::Directory => (Blue, Bold),
            DType::Symlink => (Cyan, Bold),
            DType::Regular => unsafe {
                if root.last() != Some(&b'/') {
                    root.push(b'/');
                }
                root.extend(self.name.to_bytes());
                root.push(0);
                let executable = libc::access(root.as_ptr() as *const i8, libc::X_OK);
                while root.last() != Some(&b'/') {
                    root.pop();
                }
                if executable == 0 {
                    (Green, Bold)
                } else {
                    (White, Regular)
                }
            },
            _ => (White, Regular),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let stdout = std::io::stdout();
    //let mut buf = stdout.lock();
    let mut buf = std::io::BufWriter::new(stdout.lock());
    write!(buf, "{}{}", Color::White, Style::Regular)?;

    let mut previous_color = Color::White;
    let mut previous_style = Style::Regular;
    let args = std::env::args();

    if args.len() < 2 {
        let mut root = vec![b'.'];
        for e in ReadDir::new(".").unwrap() {
            let (color, style) = e.style(&mut root);
            if color != previous_color {
                write!(buf, "{}", color)?;
                previous_color = color;
            }
            if style != previous_style {
                write!(buf, "{}", style)?;
                previous_style = style;
            }
            buf.write_all(e.name.to_bytes())?;
            buf.write_all(b"\n")?;
        }
    } else {
        for arg in args.skip(1) {
            let mut root = arg.as_bytes().iter().cloned().collect();
            for e in ReadDir::new(arg).unwrap() {
                let (color, style) = e.style(&mut root);
                if color != previous_color {
                    write!(buf, "{}", color)?;
                    previous_color = color;
                }
                if style != previous_style {
                    write!(buf, "{}", style)?;
                    previous_style = style;
                }
                buf.write_all(e.name.to_bytes())?;
                buf.write_all(b"\n")?;
            }
        }
    }

    write!(
        buf,
        "{}{}",
        termion::color::Fg(termion::color::Reset),
        termion::style::Reset
    )?;

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Color {
    White,
    Cyan,
    Blue,
    Green,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use termion::color::{Blue, Cyan, Fg, Green, White};
        match self {
            Color::White => write!(f, "{}", Fg(White)),
            Color::Cyan => write!(f, "{}", Fg(Cyan)),
            Color::Blue => write!(f, "{}", Fg(Blue)),
            Color::Green => write!(f, "{}", Fg(Green)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Regular,
    Bold,
}

impl std::fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use termion::style::{Bold, NoBold};
        match self {
            Style::Regular => write!(f, "{}", NoBold),
            Style::Bold => write!(f, "{}", Bold),
        }
    }
}
