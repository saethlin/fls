use crate::{CStr, Error};
use core::mem;
use libc::c_int;
use sc::syscall;

#[inline]
pub fn write(fd: c_int, bytes: &[u8]) -> Result<usize, Error> {
    unsafe { syscall!(WRITE, fd, bytes.as_ptr(), bytes.len()) }.usize_result()
}

// For directories RDONLY | DIRECTORY | CLOEXEC
bitflags::bitflags! {
    pub struct OpenFlags: libc::c_int {
        const RDONLY = libc::O_RDONLY;
        const WRONLY = libc::O_WRONLY;
        const RDWR = libc::O_RDWR;
        const APPEND = libc::O_APPEND;
        const ASYNC = libc::O_ASYNC;
        const CLOEXEC = libc::O_CLOEXEC;
        const CREAT = libc::O_CREAT;
        const DIRECT = libc::O_DIRECT;
        const DIRECTORY = libc::O_DIRECTORY;
        const DSYNC = libc::O_DSYNC;
        const EXCL = libc::O_EXCL;
        const LARGEFILE = libc::O_LARGEFILE;
        const NOATIME = libc::O_NOATIME;
        const NOCTTY = libc::O_NOCTTY;
        const NOFOLLOW = libc::O_NOFOLLOW;
        const NONBLOCK = libc::O_NONBLOCK;
        const PATH = libc::O_PATH;
        const SYNC = libc::O_SYNC;
        const TMPFILE = libc::O_TMPFILE;
        const TRUNC = libc::O_TRUNC;
    }
}

bitflags::bitflags! {
    pub struct OpenMode: libc::c_uint {
        const RWXU = libc::S_IRWXU;
        const RUSR = libc::S_IRUSR;
        const WUSR = libc::S_IWUSR;
        const XUSR = libc::S_IXUSR;
        const RWXG = libc::S_IRWXG;
        const RGRP = libc::S_IRGRP;
        const WGRP = libc::S_IWGRP;
        const XGRP = libc::S_IXGRP;
        const RWXO = libc::S_IRWXO;
        const ROTH = libc::S_IROTH;
        const WOTH = libc::S_IWOTH;
        const XOTH = libc::S_IXOTH;
        const SUID = libc::S_ISUID;
        const SGID = libc::S_ISGID;
        const SVTX = libc::S_ISVTX;
    }
}

#[inline]
#[cfg(target_arch = "x86_64")]
pub fn open(path: CStr, flags: OpenFlags, mode: OpenMode) -> Result<c_int, Error> {
    unsafe { syscall!(OPEN, path.as_ptr(), flags.bits(), mode.bits()) }
        .to_result_and(|n| n as c_int)
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub fn open(path: CStr, flags: OpenFlags, mode: OpenMode) -> Result<c_int, Error> {
    unsafe {
        syscall!(
            OPENAT,
            libc::AT_FDCWD,
            path.as_ptr(),
            flags.bits(),
            mode.bits()
        )
    }
    .to_result_and(|n| n as c_int)
}

#[inline]
pub fn close(fd: c_int) -> Result<(), Error> {
    unsafe { syscall!(CLOSE, fd) }.null_result()
}

#[inline]
#[cfg(target_arch = "x86_64")]
pub fn stat(path: CStr) -> Result<libc::stat, Error> {
    unsafe {
        let mut status: libc::stat = mem::zeroed();
        syscall!(STAT, path.as_ptr(), &mut status as *mut libc::stat).to_result_with(status)
    }
}

#[inline]
#[cfg(target_arch = "aarch64")]
pub fn stat(path: CStr) -> Result<libc::stat, Error> {
    unsafe {
        let fd = open(path, OpenFlags::RDONLY, OpenMode::empty())?;
        let mut status: libc::stat = mem::zeroed();
        let res = syscall!(FSTAT, fd, &mut status as *mut libc::stat).to_result_with(status);
        let _ = close(fd);
        res
    }
}

#[inline]
pub fn mmap(
    addr: *mut u8,
    len: usize,
    prot: i32,
    flags: i32,
    fd: i32,
    offset: isize,
) -> Result<*mut u8, Error> {
    unsafe { syscall!(MMAP, addr, len, prot, flags, fd, offset) }.to_result_and(|n| n as *mut u8)
}

/// munmap
///
/// # Safety
///
/// The specified memory region must not be used after this function is called
#[inline]
pub unsafe fn munmap(addr: *mut u8, len: usize) -> Result<(), Error> {
    syscall!(MUNMAP, addr, len).null_result()
}

bitflags::bitflags! {
    pub struct Mode: c_int {
        const F_OK = 0;
        const R_OK = 4;
        const W_OK = 2;
        const X_OK = 1;
    }
}

#[inline]
pub fn mremap(
    old_address: *mut u8,
    old_size: usize,
    new_size: usize,
    flags: c_int,
) -> Result<*mut u8, Error> {
    unsafe { syscall!(MREMAP, old_address, old_size, new_size, flags) }
        .to_result_and(|n| n as *mut u8)
}

#[inline]
pub fn exit(error_code: c_int) {
    unsafe {
        syscall!(EXIT, error_code);
    }
}

#[inline]
pub fn kill(pid: usize, signal: i32) -> Result<(), Error> {
    unsafe { syscall!(KILL, pid, signal) }.null_result()
}

#[inline]
pub fn fstatat(fd: c_int, name: CStr) -> Result<libc::stat64, Error> {
    unsafe {
        let mut stats = mem::zeroed();
        syscall!(
            NEWFSTATAT,
            fd,
            name.as_ptr(),
            &mut stats as *mut libc::stat64,
            0
        )
        .to_result_with(stats)
    }
}

#[inline]
pub fn lstatat(fd: c_int, name: CStr) -> Result<libc::stat64, Error> {
    unsafe {
        let mut stats = mem::zeroed();
        syscall!(
            NEWFSTATAT,
            fd,
            name.as_ptr(),
            &mut stats as *mut libc::stat64,
            libc::AT_SYMLINK_NOFOLLOW
        )
        .to_result_with(stats)
    }
}

#[inline]
pub fn getdents64(fd: c_int, buf: &mut [u8]) -> Result<usize, Error> {
    unsafe { syscall!(GETDENTS64, fd, buf.as_mut_ptr(), buf.len()) }.to_result_and(|n| n)
}

#[inline]
pub fn faccessat(fd: c_int, name: CStr, mode: c_int) -> Result<(), Error> {
    unsafe { syscall!(FACCESSAT, fd, name.as_ptr(), mode) }.null_result()
}

#[inline]
pub fn readlinkat<'a>(fd: c_int, name: CStr, buf: &'a mut [u8]) -> Result<&'a [u8], Error> {
    match unsafe { syscall!(READLINKAT, fd, name.as_ptr(), buf.as_ptr(), buf.len()) }
        .to_result_and(|n| n)
    {
        Ok(n) => Ok(buf.get(..n).unwrap_or_default()),
        Err(e) => Err(e),
    }
}

#[inline]
pub fn winsize() -> Result<libc::winsize, Error> {
    unsafe {
        let mut winsize: libc::winsize = mem::zeroed();
        syscall!(
            IOCTL,
            libc::STDOUT_FILENO,
            libc::TIOCGWINSZ,
            &mut winsize as *mut libc::winsize
        )
        .to_result_with(winsize)
    }
}

trait SyscallRet: Sized {
    fn to_result_with<T>(self, t: T) -> Result<T, Error>;
    fn to_result_and<T, F>(self, f: F) -> Result<T, Error>
    where
        F: FnOnce(Self) -> T,
        Self: Sized;

    fn usize_result(self) -> Result<usize, Error>;

    #[inline]
    fn null_result(self) -> Result<(), Error> {
        self.to_result_with(())
    }
}

impl SyscallRet for usize {
    #[inline]
    fn to_result_with<T>(self, t: T) -> Result<T, Error> {
        let ret = self as isize;
        if ret < 0 {
            Err(Error(-ret as c_int))
        } else {
            Ok(t)
        }
    }

    #[inline]
    fn to_result_and<T, F>(self, f: F) -> Result<T, Error>
    where
        F: FnOnce(Self) -> T,
        Self: Sized,
    {
        let ret = self as isize;
        if ret < 0 {
            Err(Error(-ret as c_int))
        } else {
            Ok(f(self))
        }
    }

    #[inline]
    fn usize_result(self) -> Result<usize, Error> {
        self.to_result_and(|n| n)
    }
}
