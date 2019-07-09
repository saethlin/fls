use crate::{CStr, Error, Style};
use smallvec::SmallVec;

use syscall::syscall;

const O_DIRECTORY: i32 = 0x10000;
const O_CLOEXEC: i32 = 0x80000;
const O_RDONLY: i32 = 0;

pub struct Directory {
    fd: i32,
    dirents: SmallVec<[u8; 4096]>,
    bytes_used: isize,
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            syscall!(CLOSE, self.fd);
        }
    }
}

impl<'a> Directory {
    pub fn open(path: CStr) -> Result<Self, isize> {
        // Requires that path be null-terminated, which is an invariant of CStr
        let fd =
            unsafe { syscall!(OPEN, path.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC) as isize };
        if fd < 0 {
            return Err(-fd);
        }

        let mut dirents: SmallVec<[u8; 4096]> = smallvec::smallvec![0; 4096];
        let ret = unsafe { syscall!(GETDENTS64, fd, dirents.as_mut_ptr(), dirents.len()) as isize };
        if ret < 0 {
            return Err(-ret);
        }
        let mut bytes_read = ret as usize;
        let mut bytes_used = bytes_read;

        while bytes_read > 0 {
            if dirents.len() - bytes_used < core::mem::size_of::<libc::dirent64>() {
                dirents.reserve(4096);
                dirents.extend(core::iter::repeat(0).take(4096));
            }
            let ret = unsafe {
                syscall!(
                    GETDENTS64,
                    fd,
                    dirents.as_mut_ptr().offset(bytes_used as isize),
                    dirents.len() - bytes_used as usize
                ) as isize
            };
            if ret < 0 {
                return Err(-ret);
            }
            bytes_read = ret as usize;
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
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let dirent_ptr =
                self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64;

            let entry = if self.offset < self.directory.bytes_used {
                Some(DirEntry {
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

pub struct DirEntry<'a> {
    directory: &'a Directory,
    offset: isize,
    name_len: usize,
}

impl<'a> DirEntry<'a> {
    pub fn name_with_nul(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.name_ptr() as *const u8, self.name_len + 1) }
    }

    pub fn name(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.name_ptr() as *const u8, self.name_len) }
    }

    fn d_type(&self) -> u8 {
        unsafe {
            (*(self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64)).d_type
        }
    }

    fn name_ptr(&self) -> *const libc::c_char {
        unsafe {
            let dirent_ptr =
                self.directory.dirents.as_ptr().offset(self.offset) as *const libc::dirent64;
            (*dirent_ptr).d_name.as_ptr()
        }
    }

    pub fn style(&self) -> Result<Style, Error> {
        /*
        pub enum DType {
            Fifo = 1,
            Character = 2,
            Directory = 4,
            Block = 6,
            Regular = 8,
            Symlink = 10,
            Socket = 12,
        }
        */

        match self.d_type() {
            4 => Ok(Style::BlueBold),
            10 => unsafe {
                let ret = syscall!(FACCESSAT, self.directory.fd, self.name_ptr(), 0) as isize;
                match ret {
                    0 => Ok(Style::CyanBold),
                    -2 => Ok(Style::RedBold), // ENOENT, symlink is broken
                    _ => Err(Error(ret)),
                }
            },
            8 => unsafe {
                let ret = syscall!(FACCESSAT, self.directory.fd, self.name_ptr(), 1) as isize;
                match ret {
                    0 => Ok(Style::GreenBold),
                    -13 => Ok(style_for(self.name())), // EACCESS, so we're not allowed to execute
                    _ => Err(Error(ret)),
                }
            },
            _ => Ok(Style::White),
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
    } else if document.contains(&extension) {
        Style::Magenta
    } else if media.contains(&extension) {
        Style::Magenta
    } else {
        Style::White
    }
}
