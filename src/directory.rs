use crate::syscalls;
use crate::{CStr, Error, Style};
use libc::c_int;
use smallvec::SmallVec;

pub struct Directory {
    fd: c_int,
    dirents: SmallVec<[u8; 4096]>,
    bytes_used: isize,
}

impl Directory {
    pub fn raw_fd(&self) -> c_int {
        self.fd
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let _ = syscalls::close(self.fd);
    }
}

impl<'a> Directory {
    pub fn open(path: CStr) -> Result<Self, Error> {
        let fd = syscalls::open_dir(path)?;

        let mut dirents: SmallVec<[u8; 4096]> = smallvec::smallvec![0; 4096];
        let mut bytes_read = syscalls::getdents64(fd, &mut dirents[..])?;
        let mut bytes_used = bytes_read;

        while bytes_read > 0 {
            if dirents.len() - bytes_used < core::mem::size_of::<libc::dirent64>() {
                dirents.reserve(4096);
                dirents.extend(core::iter::repeat(0).take(4096));
            }

            bytes_read = syscalls::getdents64(fd, &mut dirents[bytes_used..])?;
            bytes_used += bytes_read;
        }

        Ok(Self {
            fd: fd as i32,
            dirents,
            bytes_used: bytes_used as isize,
        })
    }

    pub fn iter(&'a self) -> IterDir<'a> {
        IterDir {
            directory: self,
            offset: 0,
        }
    }
}

pub struct IterDir<'a> {
    directory: &'a Directory,
    offset: isize,
}

impl<'a> Iterator for IterDir<'a> {
    type Item = RawDirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let dirent_ptr =
                self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64;

            let entry = if self.offset < self.directory.bytes_used {
                Some(RawDirEntry {
                    directory: self.directory,
                    offset: self.offset,
                    name_len: libc::strlen((*dirent_ptr).d_name.as_ptr()),
                })
            } else {
                None
            };

            self.offset += (*dirent_ptr).d_reclen as isize;

            entry
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            self.directory.bytes_used as usize / core::mem::size_of::<libc::dirent64>(),
            Some(
                self.directory.bytes_used as usize / (core::mem::size_of::<libc::dirent64>() - 256),
            ),
        )
    }
}

pub trait DirEntry {
    fn name(&self) -> CStr;
    fn style(&self) -> Style;
}

pub struct RawDirEntry<'a> {
    directory: &'a Directory,
    offset: isize,
    name_len: usize,
}

impl<'a> RawDirEntry<'a> {
    fn name_ptr(&self) -> *const libc::c_char {
        unsafe {
            let dirent_ptr =
                self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64;
            (*dirent_ptr).d_name.as_ptr()
        }
    }

    fn d_type(&self) -> u8 {
        unsafe {
            (*(self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64)).d_type
        }
    }
}

impl<'a> DirEntry for RawDirEntry<'a> {
    fn name(&self) -> CStr {
        let slice =
            unsafe { core::slice::from_raw_parts(self.name_ptr() as *const u8, self.name_len + 1) };
        CStr::from_bytes(slice)
    }

    fn style(&self) -> Style {
        match self.d_type() {
            libc::DT_DIR => Ok(Style::BlueBold),
            libc::DT_LNK => syscalls::faccessat(self.directory.fd, self.name(), libc::F_OK)
                .map(|_| Style::CyanBold)
                .or_else(|e| {
                    if e.0 == libc::ENOENT as isize {
                        Ok(Style::RedBold)
                    } else {
                        Err(e)
                    }
                }),
            libc::DT_REG => syscalls::faccessat(self.directory.fd, self.name(), libc::X_OK)
                .map(|_| Style::GreenBold)
                .or_else(|e| {
                    if e.0 == libc::EACCES as isize {
                        Ok(style_for(self.name().as_bytes()))
                    } else {
                        Err(e)
                    }
                }),
            libc::DT_UNKNOWN => match syscalls::lstatat(self.directory.fd, self.name()) {
                Err(e) => Err(e),
                Ok(stats) => {
                    let mode = stats.st_mode;
                    let entry_type = mode & libc::S_IFMT;
                    match entry_type {
                        libc::S_IFLNK => {
                            syscalls::faccessat(self.directory.fd, self.name(), libc::F_OK)
                                .map(|_| Style::CyanBold)
                                .or_else(|e| {
                                    if e.0 == libc::ENOENT as isize {
                                        Ok(Style::RedBold)
                                    } else {
                                        Err(e)
                                    }
                                })
                        }
                        libc::S_IFDIR => Ok(Style::BlueBold),
                        libc::S_IFREG => Ok(Style::White),
                        _ => Ok(Style::White),
                    }
                }
            },

            _ => Ok(Style::White),
        }
        .unwrap_or(Style::White)
    }
}

pub struct File<'a> {
    pub path: CStr<'a>,
}

impl<'a> DirEntry for File<'a> {
    fn name(&self) -> CStr {
        self.path
    }

    fn style(&self) -> Style {
        match syscalls::open_dir(self.path) {
            Ok(fd) => {
                let _ = syscalls::close(fd);
                Style::BlueBold
            }
            Err(Error(code)) => {
                if code == libc::ENOTDIR as isize {
                    style_for(self.name().as_bytes())
                } else {
                    Style::White
                }
            }
        }
    }
}

fn style_for(name: &[u8]) -> Style {
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::White,
        Some(ext) => ext,
    };
    let compressed: &[&[u8]] = &[b"tar", b"gz", b"tgz", b"xz"];
    let document: &[&[u8]] = &[b"pdf", b"eps"];
    let media: &[&[u8]] = &[b"png", b"mp4"];
    if compressed.contains(&extension) {
        Style::Red
    } else if document.contains(&extension) || media.contains(&extension) {
        Style::Magenta
    } else {
        Style::White
    }
}
