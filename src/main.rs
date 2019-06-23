use smallvec::SmallVec;

struct ReadDir {
    dir: *mut libc::DIR,
}

impl ReadDir {
    fn new(path: &mut Vec<u8>) -> Result<Self, std::io::Error> {
        if path.last() != Some(&0) {
            path.push(0);
        }
        unsafe {
            let dir = libc::opendir(path.as_ptr() as *const i8);
            if dir.is_null() {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(Self { dir })
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
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

    fn style(&self, root: &mut Vec<u8>) -> Result<(Color, Style), std::io::Error> {
        use Color::*;
        use Style::*;
        while root.last() == Some(&0) {
            root.pop();
        }
        match self.d_type.unwrap() {
            DType::Directory => Ok((Blue, Bold)),
            DType::Symlink => Ok((Cyan, Bold)),
            DType::Regular => unsafe {
                if root.last() != Some(&b'/') {
                    root.push(b'/');
                }
                root.extend(&self.name);
                root.push(0);
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
                        Err(std::io::Error::last_os_error())
                    }
                }
            },
            _ => Ok((White, Regular)),
        }
    }
}

fn print_entries<W: std::io::Write>(root: &mut Vec<u8>, out: &mut W) -> Result<(), std::io::Error> {
    write!(out, "{}{}", Color::White, Style::Regular)?;

    let mut previous_color = Color::White;
    let mut previous_style = Style::Regular;

    let mut entries: Vec<_> = ReadDir::new(root)?.collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    for e in entries {
        let (color, style) = e.style(root)?;
        if color != previous_color {
            write!(out, "{}", color)?;
            previous_color = color;
        }
        if style != previous_style {
            write!(out, "{}", style)?;
            previous_style = style;
        }
        out.write_all(e.name())?;
        out.write_all(b"\n")?;
    }
    write!(
        out,
        "{}{}",
        termion::color::Fg(termion::color::Reset),
        termion::style::Reset
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let args = std::env::args();

    if args.len() < 2 {
        let mut root = vec![b'.'];
        print_entries(&mut root, &mut out)?;
    } else {
        for arg in args.skip(1) {
            let mut root = arg.into_bytes();
            print_entries(&mut root, &mut out)?;
        }
    }

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
