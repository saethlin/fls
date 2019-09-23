use crate::Style;
use veneer::directory::DType;
use veneer::{syscalls, CStr};

pub trait DirEntry {
    fn name(&self) -> CStr;
    fn style(&self, dir: &veneer::Directory) -> Style;
}

impl<'a> DirEntry for veneer::directory::DirEntry<'a> {
    fn name(&self) -> CStr {
        self.name()
    }

    fn style(&self, dir: &veneer::Directory) -> Style {
        match self.d_type() {
            DType::DIR => Ok(Style::BlueBold),
            DType::LNK => syscalls::faccessat(dir.raw_fd(), self.name(), libc::F_OK)
                .map(|_| Style::CyanBold)
                .or_else(|e| {
                    if e == libc::ENOENT {
                        Ok(Style::RedBold)
                    } else {
                        Err(e)
                    }
                }),
            DType::REG => syscalls::faccessat(dir.raw_fd(), self.name(), libc::X_OK)
                .map(|_| Style::GreenBold)
                .or_else(|e| {
                    if e == libc::EACCES {
                        Ok(extension_style(self.name().as_bytes()))
                    } else {
                        Err(e)
                    }
                }),
            DType::UNKNOWN => match syscalls::lstatat(dir.raw_fd(), self.name()) {
                Err(e) => Err(e),
                Ok(stats) => {
                    let mode = stats.st_mode;
                    let entry_type = mode & libc::S_IFMT;
                    match entry_type {
                        libc::S_IFLNK => syscalls::faccessat(dir.raw_fd(), self.name(), libc::F_OK)
                            .map(|_| Style::CyanBold)
                            .or_else(|e| {
                                if e == libc::ENOENT {
                                    Ok(Style::RedBold)
                                } else {
                                    Err(e)
                                }
                            }),
                        libc::S_IFDIR => Ok(Style::BlueBold),
                        libc::S_IFREG => Ok(extension_style(self.name().as_bytes())),
                        _ => Ok(Style::White),
                    }
                }
            },

            _ => Ok(Style::White),
        }
        .unwrap_or(Style::White)
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

    fn style(&self, dir: &veneer::Directory) -> Style {
        veneer::syscalls::lstatat(dir.raw_fd(), self.path)
            .ok()
            .and_then(|status| crate::Status::from(status).style())
            .unwrap_or_else(|| extension_style(self.name().as_bytes()))
    }
}

pub fn extension_style(name: &[u8]) -> Style {
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::White,
        Some(ext) => ext,
    };
    let compressed: &[&[u8]] = &[b"tar", b"gz", b"tgz", b"xz"];
    let document: &[&[u8]] = &[b"pdf", b"eps"];
    let media: &[&[u8]] = &[b"png", b"mp4", b"jpg", b"jpeg"];
    if compressed.contains(&extension) {
        Style::Red
    } else if document.contains(&extension) || media.contains(&extension) {
        Style::Magenta
    } else {
        Style::White
    }
}

// TODO: We can use the status to get the style information we want here
impl<T> DirEntry for (T, crate::Status)
where
    T: DirEntry,
{
    fn name(&self) -> CStr {
        self.0.name()
    }

    fn style(&self, fd: &veneer::Directory) -> Style {
        self.0.style(fd)
    }
}
