use crate::cli::{App, Color, FollowSymlinks};
use crate::Style;
use veneer::directory::DType;
use veneer::{syscalls, CStr};

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
                    Link
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

pub fn extension_style(name: &[u8]) -> Style {
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::White,
        Some(ext) => ext,
    };
    let compressed: &[&[u8]] = &[b"tar", b"gz", b"tgz", b"xz"];
    let document: &[&[u8]] = &[b"pdf", b"eps", b"doc", b"docx"];
    let media: &[&[u8]] = &[b"png", b"mp3", b"mp4", b"jpg", b"jpeg", b"svg"];
    if compressed.contains(&extension) {
        Style::Red
    } else if document.contains(&extension) || media.contains(&extension) {
        Style::Magenta
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
