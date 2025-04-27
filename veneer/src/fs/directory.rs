use crate::{
    libc, syscalls,
    syscalls::{OpenFlags, OpenMode},
    CStr, Error,
};
use alloc::vec::Vec;
use core::{
    convert::TryInto,
    ffi::{c_int, c_ulong},
    mem::MaybeUninit,
};

pub struct Directory {
    fd: c_int,
}

impl Directory {
    #[inline]
    pub fn open(path: CStr) -> Result<Self, Error> {
        Ok(Self {
            fd: syscalls::openat(
                libc::AT_FDCWD,
                path,
                OpenFlags::RDONLY | OpenFlags::DIRECTORY | OpenFlags::CLOEXEC,
                OpenMode::empty(),
            )?,
        })
    }

    #[inline]
    pub fn raw_fd(&self) -> c_int {
        self.fd
    }

    // The layout from getdents64 is terrible. It's designed for use with C, which means it adds
    // padding to align everything and also null-terminates string. But *also* it stores the length
    // of each record in two different ways.
    #[inline(never)]
    fn compact(bytes: &mut [u8], mut predicate: impl FnMut(&[u8]) -> bool) -> usize {
        assert!(bytes.len() > 0);
        #[repr(C)]
        struct Header {
            inode: u64,
            offset: u64,
            reclen: u16,
            d_type: u8,
        }

        let range = bytes.as_mut_ptr_range();
        let mut read = range.start;
        let end = range.end;
        let mut write = read;
        unsafe {
            loop {
                let start = read;
                let header = read.cast::<Header>().read_unaligned();
                read = read.add(19);

                let write_start = write;

                write.cast::<u64>().write_unaligned(header.inode);
                write = write.add(8);

                *write = header.d_type;
                write = write.add(1);

                *write = 0;
                let name_len_ptr = write;
                write = write.add(1);

                let name_start = write;

                let mut name_len = 0;
                while *read != 0 {
                    name_len += 1;
                    *write = *read;
                    write = write.add(1);
                    read = read.add(1);
                }

                name_len += 1;
                *write = 0;
                write = write.add(1);

                *name_len_ptr = name_len;

                read = start.add(header.reclen as usize);

                let name = core::slice::from_raw_parts(name_start, name_len as usize);
                if !predicate(name) {
                    write = write_start;
                }

                if read == end {
                    break;
                }
            }
        }

        write.addr() - bytes.as_ptr().addr()
    }

    #[inline]
    pub fn read(
        &self,
        mut predicate: impl FnMut(&[u8]) -> bool,
    ) -> Result<DirectoryContents, Error> {
        let mut contents = Vec::with_capacity(4096);

        let bytes_used = syscalls::getdents64(self.fd, &mut contents.spare_capacity_mut())?;
        if bytes_used == 0 {
            return Ok(DirectoryContents { contents });
        }
        // SAFETY: getdents64 says that it has initialized those bytes
        unsafe {
            contents.set_len(bytes_used);
            let bytes_used = Self::compact(&mut contents, &mut predicate);
            contents.set_len(bytes_used);
        }

        // If we read something, try using the rest of the allocation
        let bytes_used = syscalls::getdents64(self.fd, contents.spare_capacity_mut())?;
        if bytes_used == 0 {
            return Ok(DirectoryContents { contents });
        }
        // SAFETY: getdents64 says that it has written to those bytes
        unsafe {
            let prev_len = contents.len();
            contents.set_len(contents.len() + bytes_used);
            let bytes_used = Self::compact(&mut contents[prev_len..], &mut predicate);
            contents.set_len(prev_len + bytes_used);
        }

        // Then, if we read something on the second time, start reallocating.
        // Must run this loop until getdents64 returns no new entries
        // Yes, even if there is plenty of unused space. Some filesystems (at least sshfs) rely on this behavior
        loop {
            contents.reserve(contents.capacity());
            let bytes_used = syscalls::getdents64(self.fd, contents.spare_capacity_mut())?;
            if bytes_used == 0 {
                return Ok(DirectoryContents { contents });
            }
            // SAFETY: getdents64 says that it has initialized those bytes
            unsafe {
                let prev_len = contents.len();
                contents.set_len(contents.len() + bytes_used);
                let bytes_used = Self::compact(&mut contents[prev_len..], &mut predicate);
                contents.set_len(prev_len + bytes_used);
            }
        }
    }
}

impl Drop for Directory {
    #[inline]
    fn drop(&mut self) {
        let _ = syscalls::close(self.fd);
    }
}

pub struct DirectoryContents {
    pub contents: Vec<u8>,
}

impl DirectoryContents {
    pub fn index(&mut self) -> BorrowedDirectoryContents<'_> {
        let (contents, index) = self.contents.split_at_spare_mut();
        let (_head, index, _tail) = unsafe { index.align_to_mut::<MaybeUninit<Index>>() };

        let mut it = IterDir {
            remaining: contents,
        };
        index[0].write(Index {
            index: 0,
            offset: 0,
        });
        let mut i = 0;
        while let Some(_entry) = it.next() {
            i += 1;
            let offset = contents.len() - it.remaining.len();
            index[i].write(Index {
                index: i as u32,
                offset: offset as u32,
            });
        }
        let index = &mut index[..i];

        unsafe {
            BorrowedDirectoryContents {
                contents,
                index: index.assume_init_mut(),
            }
        }
    }
}

pub struct BorrowedDirectoryContents<'a> {
    contents: &'a [u8],
    index: &'a mut [Index],
}

#[derive(Clone, Copy)]
struct Index {
    index: u32,
    offset: u32,
}

impl<'a> BorrowedDirectoryContents<'a> {
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = DirEntry<'a>> + '_ {
        self.index.iter().map(move |&i| {
            // SAFETY: These indexes are from our index array, so they are valid by definition.
            unsafe { Self::get_unchecked(self.contents, i.offset as usize) }
        })
    }

    pub fn iter_enumerated(&self) -> impl Iterator<Item = (usize, DirEntry<'a>)> + '_ {
        self.index.iter().map(move |&i| {
            // SAFETY: These indexes are from our index array, so they are valid by definition.
            let entry = unsafe { Self::get_unchecked(self.contents, i.offset as usize) };
            (i.index as usize, entry)
        })
    }

    pub fn sort_unstable_by<F>(&mut self, mut compare: F)
    where
        F: FnMut((usize, DirEntry<'a>), (usize, DirEntry<'a>)) -> core::cmp::Ordering,
    {
        // Mentioning self in the closure results in overlapping borrows of self and self.index.
        let contents = self.contents;
        self.index.sort_unstable_by(|&i_a, &i_b| {
            // SAFETY: These indexes are from our index array, so they are valid by definition.
            unsafe {
                let a = Self::get_unchecked(contents, i_a.offset as usize);
                let b = Self::get_unchecked(contents, i_b.offset as usize);
                compare((i_a.index as usize, a), (i_b.index as usize, b))
            }
        })
    }

    unsafe fn get_unchecked(contents: &'a [u8], i: usize) -> DirEntry<'a> {
        IterDir {
            remaining: contents.get_unchecked(i..),
        }
        .next()
        .unwrap_unchecked()
    }
}

pub struct IterDir<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for IterDir<'a> {
    type Item = DirEntry<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let (header, remaining) = unsafe { self.remaining.split_at_unchecked(10) };

        let inode = u64::from_ne_bytes(header[..8].try_into().unwrap());
        let d_type = header[8];
        let name_len = header[9] as usize;

        let (name, remaining) = unsafe { remaining.split_at_unchecked(name_len) };
        let name = unsafe { CStr::from_bytes_unchecked(name) };

        self.remaining = remaining;

        Some(DirEntry {
            inode,
            name,
            d_type: match d_type {
                0 => DType::UNKNOWN,
                1 => DType::FIFO,
                2 => DType::CHR,
                4 => DType::DIR,
                6 => DType::BLK,
                8 => DType::REG,
                10 => DType::LNK,
                12 => DType::SOCK,
                _ => DType::UNKNOWN,
            },
        })
    }
}

#[derive(Clone, Copy)]
pub struct DirEntry<'a> {
    pub inode: c_ulong,
    pub name: CStr<'a>,
    pub d_type: DType,
}

impl<'a> DirEntry<'a> {
    #[inline]
    pub fn name(&self) -> CStr<'a> {
        self.name
    }

    #[inline]
    pub fn inode(&self) -> c_ulong {
        self.inode
    }

    #[inline]
    pub fn d_type(&self) -> DType {
        self.d_type
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum DType {
    UNKNOWN = 0,
    FIFO = 1,
    CHR = 2,
    DIR = 4,
    BLK = 6,
    REG = 8,
    LNK = 10,
    SOCK = 12,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    // TODO: Get a directory with more file types

    #[test]
    fn read_cwd() {
        let dir = Directory::open(CStr::from_bytes(b"/dev\0")).unwrap();
        let contents = dir.read().unwrap();

        let mut libc_dirents = Vec::new();
        unsafe {
            let dirp = libc::opendir(b"/dev\0".as_ptr() as *const c_char);
            let mut entry = libc::readdir64(dirp);
            while !entry.is_null() {
                libc_dirents.push(*entry);
                entry = libc::readdir64(dirp);
            }
            libc::closedir(dirp);
        }

        for (libc, ven) in libc_dirents.iter().zip(contents.iter()) {
            unsafe {
                assert_eq!(CStr::from_ptr(libc.d_name.as_ptr().cast()), ven.name);
            }
            assert_eq!(libc.d_ino, ven.inode);
            assert_eq!(libc.d_type, ven.d_type as u8);
        }
    }
}
