use crate::error::Error;

use smallvec::SmallVec;
use syscall::syscall;

const O_DIRECTORY: i32 = 0x10000;
const O_CLOEXEC: i32 = 0x80000;
const O_RDONLY: i32 = 0;

pub struct Directory {
    fd: i32,
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            syscall!(CLOSE, self.fd);
        }
    }
}

impl<'a> Directory {
    pub fn open(path: &[u8]) -> Result<Self, i32> {
        if path.last() != Some(&0) {
            return Err(1337);
        }
        // Requires that path be null-terminated, which we check above
        let ret =
            unsafe { syscall!(OPEN, path.as_ptr(), O_RDONLY | O_DIRECTORY | O_CLOEXEC) as i32 };
        if ret < 0 {
            Err(-ret)
        } else {
            Ok(Self { fd: ret })
        }
    }

    pub fn iter(&'a self) -> Result<IterDir<'a>, i32> {
        let mut this = IterDir {
            directory: self,
            buf: [0u8; 32768],
            bytes_read: 0,
            offset: 0,
        };
        // Requires that the length be correct, which it is by construction
        let ret =
            unsafe { syscall!(GETDENTS64, self.fd, this.buf.as_mut_ptr(), this.buf.len()) as i32 };
        if ret < 0 {
            Err(-ret)
        } else {
            this.bytes_read = ret as isize;
            Ok(this)
        }
    }
}

pub struct IterDir<'a> {
    directory: &'a Directory,
    buf: [u8; 32768],
    bytes_read: isize,
    offset: isize,
}

#[repr(C)]
struct linux_dirent {
    d_ino: i64,
    d_off: i64,
    d_reclen: u16,
    d_type: u8,
    d_name: [u8; 256],
}

impl<'a> Iterator for IterDir<'a> {
    type Item = DirEntry<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.bytes_read == 0 {
                return None;
            }
            // We've run through our previous call, refill the buffer
            if self.offset == self.bytes_read {
                self.offset = 0;
                self.bytes_read = 0;
                let ret = syscall!(
                    GETDENTS64,
                    self.directory.fd,
                    self.buf.as_mut_ptr(),
                    self.buf.len()
                ) as i32;
                if ret < 0 {
                    //TODO: Report the error
                    return None;
                } else {
                    self.bytes_read = ret as isize;
                }
            }
            // Check if the attempt to refresh didn't actually get us anything
            if self.bytes_read == 0 {
                return None;
            }
            // TODO: This code could be parsing the bytes in the buffer intead of using pointer
            // casting, which would be much safer
            let dirent = &*(self.buf.as_ptr().offset(self.offset) as *const linux_dirent);
            self.offset += dirent.d_reclen as isize;
            let mut name = SmallVec::new();
            for d in dirent.d_name.iter() {
                name.push(*d);
                if *d == 0 {
                    break;
                }
            }
            let d_type = match dirent.d_type {
                0 => None,
                1 => Some(DType::Fifo),
                2 => Some(DType::Character),
                4 => Some(DType::Directory),
                6 => Some(DType::Block),
                8 => Some(DType::Regular),
                10 => Some(DType::Symlink),
                12 => Some(DType::Socket),
                _ => None,
            };
            Some(DirEntry {
                directory: self.directory,
                name,
                d_type,
            })
        }
    }
}

pub struct DirEntry<'a> {
    directory: &'a Directory,
    name: SmallVec<[u8; 24]>,
    d_type: Option<DType>,
}

pub enum DType {
    Fifo = 1,
    Character = 2,
    Directory = 4,
    Block = 6,
    Regular = 8,
    Symlink = 10,
    Socket = 12,
}

impl<'a> DirEntry<'a> {
    pub fn name_with_nul(&self) -> &[u8] {
        &self.name
    }

    pub fn name(&self) -> &[u8] {
        &self.name[..self.name.len() - 1]
    }

    pub fn style(&self) -> Result<Style, Error> {
        match self.d_type {
            Some(DType::Directory) => Ok(Style::Directory),
            Some(DType::Symlink) => unsafe {
                let ret = syscall!(FACCESSAT, self.directory.fd, self.name.as_ptr(), 0) as i32;
                match ret {
                    0 => Ok(Style::Symlink),
                    -2 => Ok(Style::BrokenSymlink), // ENOENT, symlink is broken
                    _ => Err(Error(ret)),
                }
            },
            Some(DType::Regular) => unsafe {
                let ret = syscall!(FACCESSAT, self.directory.fd, self.name.as_ptr(), 1) as i32;
                match ret {
                    0 => Ok(Style::Executable),
                    -13 => Ok(style_for(self.name())), // EACCESS, so we're not allowed to execute
                    _ => Err(Error(ret)),
                }
            },
            _ => Ok(Style::Regular),
        }
    }
}

fn style_for(name: &[u8]) -> Style {
    let extension = match name.rsplit(|b| *b == b'.').next() {
        None => return Style::Regular,
        Some(ext) => ext,
    };
    let compressed: [&'static [u8]; 2] = [b"tar", b"gz"];
    let document: [&'static [u8]; 2] = [b"pdf", b"eps"];
    let media: [&'static [u8]; 2] = [b"png", b"mp4"];
    if compressed.contains(&extension) {
        Style::Compressed
    } else if document.contains(&extension) {
        Style::Document
    } else if media.contains(&extension) {
        Style::Media
    } else {
        Style::Regular
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Regular,
    Directory,
    Executable,
    BrokenSymlink,
    Symlink,
    Compressed,
    Document,
    Media,
    Yellow,
}
