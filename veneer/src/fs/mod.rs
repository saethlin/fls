use crate::{
    io::{Read, Write},
    libc, syscalls,
    syscalls::{OpenFlags, OpenMode},
    CStr, Error,
};
use alloc::{vec, vec::Vec};
use core::ffi::c_int;

mod directory;
pub use directory::*;

pub struct File(c_int);

impl File {
    #[inline]
    pub fn open(path: &[u8]) -> Result<Self, Error> {
        Ok(Self(syscalls::openat(
            libc::AT_FDCWD,
            CStr::from_bytes(path),
            OpenFlags::RDONLY | OpenFlags::CLOEXEC,
            OpenMode::empty(),
        )?))
    }

    #[inline]
    pub fn create(path: &[u8]) -> Result<Self, Error> {
        Ok(Self(syscalls::openat(
            libc::AT_FDCWD,
            CStr::from_bytes(path),
            OpenFlags::RDWR | OpenFlags::CREAT | OpenFlags::CLOEXEC,
            OpenMode::RUSR
                | OpenMode::WUSR
                | OpenMode::RGRP
                | OpenMode::WGRP
                | OpenMode::ROTH
                | OpenMode::WOTH,
        )?))
    }
}

impl Drop for File {
    #[inline]
    fn drop(&mut self) {
        let _ = syscalls::close(self.0);
    }
}

impl Read for File {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        syscalls::read(self.0, buf)
    }
}

impl Write for File {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize, Error> {
        syscalls::write(self.0, buf)
    }
}

#[inline]
pub fn read(path: &[u8]) -> Result<Vec<u8>, Error> {
    let file_len =
        syscalls::fstatat(libc::AT_FDCWD, CStr::from_bytes(path)).map(|stat| stat.st_size)?;
    let mut file = File::open(path)?;
    let mut bytes = vec![0; file_len as usize];
    let mut buf = &mut bytes[..];
    while !buf.is_empty() {
        match file.read(buf) {
            Ok(0) => break,
            Ok(n) => buf = &mut buf[n..],
            Err(Error(libc::EAGAIN)) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files() {
        let expected_contents = &b"test contents\n"[..];

        let mut file = File::create(b"/tmp/test.foo\0").unwrap();
        file.write(expected_contents).unwrap();

        let mut contents = [0; 64];
        let mut file = File::open(b"/tmp/test.foo\0").unwrap();
        let bytes_read = file.read(&mut contents).unwrap();

        assert_eq!(&contents[..bytes_read], expected_contents);

        let contents = read(b"/tmp/test.foo\0").unwrap();
        assert_eq!(contents, expected_contents);
    }
}
