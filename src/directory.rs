use crate::{
    cli::{App, Color, FollowSymlinks},
    veneer,
    veneer::{directory::DType, syscalls, CStr},
    Style,
};

pub trait DirEntryExt {
    fn name(&self) -> CStr;
    fn style(&self, dir: &veneer::Directory, app: &App) -> (Style, Option<u8>);
    fn inode(&self) -> u64;
    fn blocks(&self) -> u64;
    fn time(&self) -> libc::time_t;
    fn d_type(&self) -> DType;
    fn size(&self) -> libc::off_t;
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
            (Socket, _) => (Some(MagentaBold), None),
            (Fifo | Other, _) => (Some(YellowBold), None),
        }
    }
}

impl<'a> DirEntryExt for (veneer::directory::DirEntry<'a>, Option<crate::Status>) {
    fn name(&self) -> CStr {
        self.0.name
    }

    fn inode(&self) -> u64 {
        self.0.inode
    }

    fn blocks(&self) -> u64 {
        if let Some(st) = &self.1 {
            st.blocks as u64
        } else {
            0
        }
    }

    fn time(&self) -> libc::time_t {
        if let Some(st) = &self.1 {
            st.time
        } else {
            0
        }
    }

    fn size(&self) -> libc::off_t {
        if let Some(st) = &self.1 {
            st.size
        } else {
            0
        }
    }

    fn d_type(&self) -> DType {
        self.0.d_type
    }

    fn style(&self, dir: &veneer::Directory, app: &App) -> (Style, Option<u8>) {
        use EntryType::*;

        // Deduce the correct DirEntry type
        let entry_type = if let Some(status) = &self.1 {
            entry_type_from_status(status)
        } else if app.color == Color::Never && app.suffixes == crate::cli::Suffixes::None {
            // DO nothing extra if no colors and no suffixes are required
            Regular
        } else if app.color == Color::Auto {
            match self.0.d_type {
                DType::DIR => Directory,
                DType::FIFO => Fifo,
                DType::SOCK => Socket,
                DType::CHR | DType::BLK => Other,
                DType::LNK => Link,
                DType::REG | DType::UNKNOWN => Regular,
            }
        } else {
            match self.0.d_type {
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
                    syscalls::fstatat(dir.raw_fd(), self.0.name)
                } else {
                    syscalls::lstatat(dir.raw_fd(), self.0.name)
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
                            && syscalls::faccessat(dir.raw_fd(), self.0.name, libc::F_OK).is_err()
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
        };

        let (style, suffix) = entry_type.style(app);

        if app.color == Color::Never {
            return (Style::Reset, suffix);
        }

        if let Some(style) = style {
            (style, suffix)
        } else {
            (extension_style(self.name().as_bytes()), suffix)
        }
    }
}

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

pub fn extension_style(name: &[u8]) -> Style {
    if name.get(0) == Some(&b'#') || name.last() == Some(&b'~') || name.last() == Some(&b'#') {
        return Style::Fixed(244);
    }
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::White,
        Some(ext) => ext,
    };
    if let Ok(i) = EXTENSION_STYLES.binary_search_by(|probe| probe.0.cmp(extension)) {
        EXTENSION_STYLES[i].1
    } else {
        Style::White
    }
}

fn entry_type_from_status(status: &crate::Status) -> EntryType {
    use EntryType::*;
    let entry_type = status.mode & libc::S_IFMT;
    if entry_type == libc::S_IFDIR {
        Directory
    } else if entry_type == libc::S_IFIFO {
        Fifo
    } else if entry_type == libc::S_IFSOCK {
        Socket
    } else if entry_type == libc::S_IFLNK {
        Link
    } else if status.mode & libc::S_IXUSR > 0 {
        Executable
    } else if entry_type == libc::S_IFREG {
        Regular
    } else {
        Other
    }
}
