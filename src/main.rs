use std::fs;
use std::path::Path;
use std::io::Write;
use rayon::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Red,
    Cyan,
    Blue,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use termion::color::{White, Red, Cyan, Blue, Fg};
        match self {
            Color::White => write!(f, "{}", Fg(White)),
            Color::Red => write!(f, "{}", Fg(Red)),
            Color::Cyan => write!(f, "{}", Fg(Cyan)),
            Color::Blue => write!(f, "{}", Fg(Blue)),
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
        use termion::style::{Reset, Bold};
        match self {
            Style::Regular => write!(f, "{}", Reset),
            Style::Bold => write!(f, "{}", Bold),
        }
    }
}



fn main() -> Result<(), Box<dyn std::error::Error>> {

    let stdout = std::io::stdout();
    let mut buf = std::io::BufWriter::new(stdout.lock());

    let arg = std::env::args().nth(1);
    let target = arg.as_ref().map(|s| s.as_str()).unwrap_or(".");

    let entries: Result<Vec<_>, _> =
        fs::read_dir(target)?.map(|r| r.map(|e| e.path())).collect();
    let entries = entries?;

    let mut output = Vec::with_capacity(entries.len());
    entries.par_iter().map(|path| {
        let mut color = Color::White;
        let mut style = Style::Regular;
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() {
                if fs::metadata(&path).is_err() {
                    color = Color::Red;
                } else {
                    color = Color::Cyan;
                }
            } else if metadata.file_type().is_dir() {
                color = Color::Blue;
                style = Style::Bold;
            }
        }
        (style, color, path)
    }).collect_into_vec(&mut output);

    let mut previous = (Style::Regular, Color::White);
    for (style, color, name) in output {
        let path: &Path = name.file_name().unwrap().as_ref();
        if style != previous.0 {
            write!(buf, "{}", style)?;
        }
        if color != previous.1 {
            write!(buf, "{}", color)?;
        }
        writeln!(buf, "{}", path.display())?;

        previous = (style, color);
    }
    write!(buf, "{}{}", termion::color::Fg(termion::color::Reset), termion::style::Reset)?;

    Ok(())
}
