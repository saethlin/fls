use crate::{CStr, Error};
use libc::c_int;

use syscall::syscall;

pub fn write(fd: c_int, bytes: &[u8]) -> Result<usize, Error> {
    let ret = unsafe { syscall!(WRITE, fd, bytes.as_ptr(), bytes.len()) } as isize;
    ret.to_result(ret as usize)
}

pub fn close(fd: c_int) -> Result<(), Error> {
    let ret = unsafe { syscall!(CLOSE, fd) } as isize;
    ret.to_result(())
}

pub fn open_dir(path: CStr) -> Result<c_int, Error> {
    let ret = unsafe {
        syscall!(
            OPEN,
            path.as_ptr(),
            libc::O_RDONLY,
            libc::O_DIRECTORY,
            libc::O_CLOEXEC
        )
    } as isize;
    ret.to_result(ret as c_int)
}

// TODO: This should take a CStr
pub fn lstat(path: &[u8], stats: &mut libc::stat64) -> Result<(), Error> {
    if path.last() != Some(&0) {
        return Err(libc::EFAULT)?;
    }
    let ret = unsafe { syscall!(LSTAT, path.as_ptr(), stats as *mut libc::stat64) } as isize;
    ret.to_result(())
}

pub fn getdents64(fd: c_int, buf: &mut [u8]) -> Result<usize, Error> {
    let ret = unsafe { syscall!(GETDENTS64, fd, buf.as_mut_ptr(), buf.len()) } as isize;
    ret.to_result(ret as usize)
}

// TODO: This should take a CStr
pub fn faccessat(fd: c_int, name: &[u8], mode: c_int) -> Result<(), Error> {
    let ret = unsafe { syscall!(FACCESSAT, fd, name.as_ptr(), mode) } as isize;
    ret.to_result(())
}

pub fn winsize() -> Result<libc::winsize, Error> {
    unsafe {
        let mut winsize: libc::winsize = core::mem::zeroed();
        let ret = syscall::syscall!(
            IOCTL,
            libc::STDOUT_FILENO,
            libc::TIOCGWINSZ,
            &mut winsize as *mut libc::winsize
        ) as isize;
        ret.to_result(winsize)
    }
}

trait ErrorCode {
    fn to_result<T>(self, t: T) -> Result<T, Error>;
}

impl ErrorCode for isize {
    fn to_result<T>(self, t: T) -> Result<T, Error> {
        if self < 0 {
            Err(Error(-self))
        } else {
            Ok(t)
        }
    }
}
