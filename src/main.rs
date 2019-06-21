use rayon::prelude::*;
use std::fs;
use std::path::Path;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "fls", about = "Experiments in a fast ls implementation")]
struct Opt {
    #[structopt(parse(from_os_str))]
    files: Vec<PathBuf>,
    #[structopt(short = "R", long = "recursive")]
    recurse: bool,
}

fn get_entries<P: AsRef<Path>>(dir: P, output: &mut Vec<DirEntry>) -> Result<(), std::io::Error> {
    let entries: Result<Vec<_>, _> = fs::read_dir(dir)?.map(|r| r.map(|e| e.path())).collect();
    let entries = entries?;

    output.reserve(entries.len());

    entries
        .into_par_iter()
        .map(|path| {
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
            DirEntry { style, color, path }
        })
        .collect_into_vec(output);

    Ok(())
}

fn print_contents_of<P: AsRef<Path>, W: std::io::Write>(
    dir: P,
    w: &mut W,
    entries: &mut Vec<DirEntry>,
    recurse: bool,
) -> Result<(), std::io::Error> {
    get_entries(dir, entries)?;

    let mut previous_style = Style::Regular;
    let mut previous_color = Color::White;
    for DirEntry { style, color, path } in entries.iter() {
        let path: &Path = path.file_name().unwrap().as_ref();
        if *style != previous_style {
            write!(w, "{}", *style)?;
            previous_style = *style;
        }
        if *color != previous_color {
            write!(w, "{}", *color)?;
            previous_color = *color;
        }
        writeln!(w, "{}", path.display())?;
    }
    write!(
        w,
        "{}{}",
        termion::color::Fg(termion::color::Reset),
        termion::style::Reset
    )?;

    if recurse {
        let dirs = entries
            .drain(..)
            .filter_map(|e| {
                if e.color == Color::Blue {
                    Some(e.path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for d in dirs {
            writeln!(w, "\n{}", d.display())?;
            print_contents_of(d, w, entries, true)?;
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let opt = Opt::from_args();

    let stdout = std::io::stdout();
    let mut buf = std::io::BufWriter::new(stdout.lock());

    let mut entries = Vec::new();
    if opt.files.is_empty() {
        print_contents_of(".", &mut buf, &mut entries, opt.recurse)?;
    } else if opt.files.len() == 1 {
        print_contents_of(&opt.files[0], &mut buf, &mut entries, opt.recurse)?;
    } else {
        for target in &opt.files {
            writeln!(&mut buf, "{}", target.display())?;
            print_contents_of(target, &mut buf, &mut entries, opt.recurse)?;
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

struct DirEntry {
    path: PathBuf,
    color: Color,
    style: Style,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Red,
    Cyan,
    Blue,
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use termion::color::{Blue, Cyan, Fg, Red, White};
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
        use termion::style::{Bold, Reset};
        match self {
            Style::Regular => write!(f, "{}", Reset),
            Style::Bold => write!(f, "{}", Bold),
        }
    }
}
