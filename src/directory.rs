use crate::{
    cli::{App, Color, FollowSymlinks},
    Style,
};
use veneer::{directory::DType, syscalls, CStr};

pub trait DirEntry {
    fn name(&self) -> CStr;
    fn style(&self, dir: &veneer::Directory, app: &App) -> (Style, Option<u8>);
    fn inode(&self) -> u64;
    fn blocks(&self) -> u64;
}

#[derive(Clone, Copy)]
pub enum EntryType {
    Directory,
    Executable,
    Regular,
    Link,
    BrokenLink,
    Fifo,
    Socket,
    Other,
}

impl EntryType {
    fn style(self, app: &App) -> (Option<Style>, Option<u8>) {
        use crate::cli::Suffixes;
        use EntryType::*;
        use Style::*;
        match (self, app.suffixes) {
            (Directory, Suffixes::None) => (Some(BlueBold), None),
            (Directory, _) => (Some(BlueBold), Some(b'/')),
            (Executable, Suffixes::All) => (Some(GreenBold), Some(b'*')),
            (Executable, _) => (Some(GreenBold), None),
            (Regular, _) => (None, None),
            (Link, Suffixes::All) => (Some(CyanBold), Some(b'@')),
            (Link, _) => (Some(CyanBold), None),
            (BrokenLink, Suffixes::All) => (Some(RedBold), Some(b'@')),
            (BrokenLink, _) => (Some(RedBold), None),
            (Fifo, Suffixes::All) => (Some(YellowBold), Some(b'|')),
            (Fifo, _) => (Some(YellowBold), None),
            (Socket, _) => (Some(MagentaBold), None),
            (Other, _) => (Some(YellowBold), None),
        }
    }
}

impl<'a> DirEntry for veneer::directory::DirEntry<'a> {
    fn name(&self) -> CStr {
        self.name()
    }

    fn inode(&self) -> u64 {
        self.inode()
    }

    fn blocks(&self) -> u64 {
        0
    }

    fn style(&self, dir: &veneer::Directory, app: &App) -> (Style, Option<u8>) {
        use EntryType::*;
        if app.color == Color::Never {
            return (Style::White, None);
        } else if app.color == Color::Auto {
            let (style, suffix) = match self.d_type() {
                DType::DIR => Directory,
                DType::FIFO => Fifo,
                DType::SOCK => Socket,
                DType::CHR | DType::BLK => Other,
                DType::LNK => Link,
                DType::REG | DType::UNKNOWN => Regular,
            }
            .style(app);

            return if let Some(style) = style {
                (style, suffix)
            } else {
                (extension_style(self.name().as_bytes()), suffix)
            };
        }
        let (style, suffix) = match self.d_type() {
            DType::DIR => Directory,
            DType::FIFO => Fifo,
            DType::SOCK => Socket,
            DType::CHR | DType::BLK => Other,
            DType::REG => syscalls::faccessat(dir.raw_fd(), self.name(), libc::X_OK)
                .map(|_| Executable)
                .unwrap_or(Regular),
            DType::LNK => syscalls::faccessat(dir.raw_fd(), self.name(), libc::F_OK)
                .map(|_| Link)
                .unwrap_or(BrokenLink),
            DType::UNKNOWN => if app.follow_symlinks == FollowSymlinks::Always {
                syscalls::fstatat(dir.raw_fd(), self.name())
            } else {
                syscalls::lstatat(dir.raw_fd(), self.name())
            }
            .map(|status| {
                let status = app.convert_status(status);
                let entry_type = status.mode & libc::S_IFMT;
                if entry_type == libc::S_IFDIR {
                    Directory
                } else if entry_type == libc::S_IFIFO {
                    Fifo
                } else if entry_type == libc::S_IFLNK {
                    if app.color == Color::Always
                        && syscalls::faccessat(dir.raw_fd(), self.name(), libc::F_OK).is_err()
                    {
                        BrokenLink
                    } else {
                        Link
                    }
                } else if status.mode & libc::S_IXUSR > 0 {
                    Executable
                } else {
                    Regular
                }
            })
            .unwrap_or(BrokenLink),
        }
        .style(app);

        if let Some(style) = style {
            (style, suffix)
        } else {
            (extension_style(self.name().as_bytes()), suffix)
        }
    }
}

#[derive(Clone)]
pub struct File<'a> {
    pub path: CStr<'a>,
}

impl<'a> DirEntry for File<'a> {
    fn name(&self) -> CStr {
        self.path
    }

    fn inode(&self) -> u64 {
        0
    }

    fn blocks(&self) -> u64 {
        0
    }

    fn style(&self, dir: &veneer::Directory, app: &App) -> (Style, Option<u8>) {
        use EntryType::*;
        if app.color == Color::Never {
            return (Style::White, None);
        } else if app.color == Color::Auto {
            return (extension_style(self.name().as_bytes()), None);
        }
        let entry_type = if app.follow_symlinks == FollowSymlinks::Always {
            syscalls::fstatat(dir.raw_fd(), self.name()).map(|s| app.convert_status(s))
        } else {
            syscalls::lstatat(dir.raw_fd(), self.name()).map(|s| app.convert_status(s))
        }
        .map(|status| {
            let entry_type = status.mode & libc::S_IFMT;
            if entry_type == libc::S_IFDIR {
                Directory
            } else if entry_type == libc::S_IFIFO {
                Fifo
            } else if entry_type == libc::S_IFLNK {
                Link
            } else if status.mode & libc::S_IXUSR > 0 {
                Executable
            } else {
                Regular
            }
        })
        .unwrap_or(BrokenLink);

        match entry_type.style(app) {
            (Some(style), suffix) => (style, suffix),
            (None, suffix) => (extension_style(self.name().as_bytes()), suffix),
        }
    }
}

static IMAGE: &[&[u8]] = &[
    b"png", b"jpeg", b"jpg", b"gif", b"bmp", b"tiff", b"tif", b"ppm", b"pgm", b"pbm", b"pnm",
    b"webp", b"raw", b"arw", b"svg", b"stl", b"eps", b"dvi", b"ps", b"cbr", b"jpf", b"cbz", b"xpm",
    b"ico", b"cr2", b"orf", b"nef", b"heif",
];

static VIDEO: &[&[u8]] = &[
    b"avi", b"flv", b"m2v", b"m4v", b"mkv", b"mov", b"mp4", b"mpeg", b"mpg", b"ogm", b"ogv",
    b"vob", b"wmv", b"webm", b"m2ts", b"heic",
];

static MUSIC: &[&[u8]] = &[b"aac", b"m4a", b"mp3", b"ogg", b"wma", b"mka", b"opus"];

static LOSSLESS: &[&[u8]] = &[b"alac", b"ape", b"flac", b"wav"];

static CRYPTO: &[&[u8]] = &[
    b"asc",
    b"enc",
    b"gpg",
    b"pgp",
    b"sig",
    b"signature",
    b"pfx",
    b"p12",
];

static DOCUMENT: &[&[u8]] = &[
    b"djvu", b"doc", b"docx", b"dvi", b"eml", b"eps", b"fotd", b"key", b"keynote", b"numbers",
    b"odp", b"odt", b"pages", b"pdf", b"ppt", b"pptx", b"rtf", b"xls", b"xlsx",
];

static COMPRESSED: &[&[u8]] = &[
    b"zip", b"tar", b"Z", b"z", b"gz", b"bz2", b"a", b"ar", b"7z", b"iso", b"dmg", b"tc", b"rar",
    b"par", b"tgz", b"xz", b"txz", b"lz", b"tlz", b"lzma", b"deb", b"rpm", b"zst",
];

static TEMP: &[&[u8]] = &[b"tmp", b"swp", b"swo", b"swn", b"bak", b"bk"];

pub fn extension_style(name: &[u8]) -> Style {
    if name.get(0) == Some(&b'#') || name.last() == Some(&b'~') || name.last() == Some(&b'#') {
        return Style::Fixed(244);
    }
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::White,
        Some(ext) => ext,
    };
    if TEMP.contains(&extension) {
        Style::Fixed(244)
    } else if IMAGE.contains(&extension) {
        Style::Fixed(133)
    } else if VIDEO.contains(&extension) {
        Style::Fixed(135)
    } else if MUSIC.contains(&extension) {
        Style::Fixed(92)
    } else if LOSSLESS.contains(&extension) {
        Style::Fixed(93)
    } else if CRYPTO.contains(&extension) {
        Style::Fixed(109)
    } else if DOCUMENT.contains(&extension) {
        Style::Fixed(105)
    } else if COMPRESSED.contains(&extension) {
        Style::Red
    } else {
        Style::White
    }
}

impl<T> DirEntry for (T, crate::Status)
where
    T: DirEntry,
{
    fn name(&self) -> CStr {
        self.0.name()
    }

    fn inode(&self) -> u64 {
        self.1.inode
    }

    fn blocks(&self) -> u64 {
        self.1.blocks as u64
    }

    fn style(&self, _fd: &veneer::Directory, app: &App) -> (Style, Option<u8>) {
        use EntryType::*;
        let entry_type = self.1.mode & libc::S_IFMT;
        let (style, suffix) = if entry_type == libc::S_IFDIR {
            Directory
        } else if entry_type == libc::S_IFIFO {
            Fifo
        } else if entry_type == libc::S_IFSOCK {
            Socket
        } else if entry_type == libc::S_IFLNK {
            Link
        } else if self.1.mode & libc::S_IXUSR > 0 {
            Executable
        } else if entry_type == libc::S_IFREG {
            Regular
        } else {
            Other
        }
        .style(app);

        if let Some(style) = style {
            (style, suffix)
        } else {
            (extension_style(self.name().as_bytes()), suffix)
        }
    }
}
