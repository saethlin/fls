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

// TODO: Reducing the number of escape sequences that's printed is worth ~10% perf
// But right now there seems to be an alacritty bug with resetting colors after a newline
fn write_grid<W: std::io::Write>(
    root: &mut Vec<u8>,
    out: &mut W,
    terminal_width: usize,
) -> Result<(), std::io::Error> {
    let mut entries: Vec<_> = ReadDir::new(root)?.collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

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
        for c in 0..columns {
            if c * rows + r > entries.len() - 1 {
                break;
            }
            let e = &entries[c * rows + r];

            let (color, style) = e.style(root)?;
            write!(out, "{}{}", color, style)?;
            out.write_all(e.name())?;
            write!(
                out,
                "{}{}",
                termion::color::Fg(termion::color::Reset),
                termion::style::Reset,
            )?;

            for _ in 0..widths[c] - e.name().len() {
                out.write_all(b" ")?;
            }
        }
        out.write_all(b"\n")?;
    }

    Ok(())
}

fn write_single_column<W: std::io::Write>(root: &mut Vec<u8>, out: &mut W) -> std::io::Result<()> {
    let mut entries: Vec<_> = ReadDir::new(root)?.collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    for e in entries {
        out.write_all(e.name())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let args = std::env::args();

    let terminal_width = termion::terminal_size().map(|s| s.0 as usize);

    if args.len() < 2 {
        let mut root = vec![b'.'];
        if let Ok(width) = terminal_width {
            write_grid(&mut root, &mut out, width)?;
        } else {
            write_single_column(&mut root, &mut out)?;
        }
    } else {
        for arg in args.skip(1) {
            let mut root = arg.into_bytes();
            if let Ok(width) = terminal_width {
                write_grid(&mut root, &mut out, width)?;
            } else {
                write_single_column(&mut root, &mut out)?;
            }
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
