use crate::{libc, CStr, Error};
use core::{
    ffi::{c_int, c_uint},
    marker::PhantomData,
    mem,
    mem::MaybeUninit,
};
use sc::syscall;

#[inline]
pub fn read(fd: c_int, bytes: &mut [u8]) -> Result<usize, Error> {
    unsafe { syscall!(READ, fd, bytes.as_mut_ptr(), bytes.len()) }.usize_result()
}

#[inline]
pub fn write(fd: c_int, bytes: &[u8]) -> Result<usize, Error> {
    unsafe { syscall!(WRITE, fd, bytes.as_ptr(), bytes.len()) }.usize_result()
}

// For directories RDONLY | DIRECTORY | CLOEXEC
bitflags::bitflags! {
    pub struct OpenFlags: c_int {
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
    pub struct OpenMode: c_uint {
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
pub fn openat(at_fd: c_int, path: CStr, flags: OpenFlags, mode: OpenMode) -> Result<c_int, Error> {
    unsafe { syscall!(OPENAT, at_fd, path.as_ptr(), flags.bits(), mode.bits()) }
        .to_result_and(|n| n as c_int)
}

#[inline]
pub fn close(fd: c_int) -> Result<(), Error> {
    unsafe { syscall!(CLOSE, fd) }.null_result()
}

#[inline]
pub fn fstat(fd: c_int) -> Result<libc::stat, Error> {
    unsafe {
        let mut status: libc::stat = mem::zeroed();
        syscall!(FSTAT, fd, &mut status as *mut libc::stat).to_result_with(status)
    }
}

#[inline]
pub fn lstat(path: CStr) -> Result<libc::stat, Error> {
    unsafe {
        let mut status: libc::stat = mem::zeroed();
        syscall!(FSTAT, path.as_ptr(), &mut status as *mut libc::stat).to_result_with(status)
    }
}

#[inline]
pub fn ppoll(
    fds: &mut [libc::pollfd],
    timeout: &libc::timespec,
    sigmask: &libc::sigset_t,
) -> Result<usize, Error> {
    unsafe {
        syscall!(
            PPOLL,
            fds.as_mut_ptr(),
            fds.len(),
            timeout as *const libc::timespec,
            sigmask as *const libc::sigset_t
        )
    }
    .usize_result()
}

#[derive(Clone, Copy)]
pub enum SeekFrom {
    Start,
    End,
    Current,
}

#[inline]
pub fn lseek(fd: c_int, seek_mode: SeekFrom, offset: usize) -> Result<usize, Error> {
    let seek_mode = match seek_mode {
        SeekFrom::Start => libc::SEEK_SET,
        SeekFrom::End => libc::SEEK_END,
        SeekFrom::Current => libc::SEEK_CUR,
    };
    unsafe { syscall!(LSEEK, fd, seek_mode, offset) }.usize_result()
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

#[inline]
pub fn mprotect(memory: &[u8], protection: c_int) -> Result<(), Error> {
    unsafe { syscall!(MPROTECT, memory.as_ptr(), memory.len(), protection) }.null_result()
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

#[inline]
pub fn brk(addr: *mut u8) -> Result<*mut u8, Error> {
    unsafe { syscall!(BRK, addr) }.to_result_and(|n| n as *mut u8)
}

/*
// Wraps the rt_sigaction call in the same way that glibc does
// So I guess there's no way to use normal signals, only realtime signals?
#[inline]
pub fn sigaction(
    signal: c_int,
    action: &libc::sigaction,
    old_action: &mut libc::sigaction,
) -> Result<(), Error> {
    unsafe {
        syscall!(
            RT_SIGACTION,
            signal,
            action as *const libc::sigaction,
            old_action as *mut libc::sigaction,
            mem::size_of::<libc::sigset_t>()
        )
    }
    .to_result_with(())
}
*/

// sigprocmask

// sigreturn

#[macro_export]
macro_rules! ioctl {
    ($fd:expr, $request:expr, $($arg:expr),*) => {
        unsafe { syscall!(IOCTL, $fd, $request, $($arg)*) }.usize_result()
    };
    ($fd:expr, $request:expr, $($arg:expr),*) => {
        ioctl!($fd, $request, $($arg)*)
    };
}

#[inline]
pub fn pread64(fd: c_int, buf: &mut [u8], offset: usize) -> Result<usize, Error> {
    unsafe { syscall!(PREAD64, fd, buf.as_mut_ptr(), buf.len(), offset) }.usize_result()
}

#[inline]
pub fn pwrite64(fd: c_int, buf: &[u8], offset: usize) -> Result<usize, Error> {
    unsafe { syscall!(PWRITE64, fd, buf.as_ptr(), buf.len(), offset) }.usize_result()
}

pub struct IoVec<'a> {
    #[allow(dead_code)]
    iov_base: *mut u8,
    #[allow(dead_code)]
    iov_len: usize,
    _danny: PhantomData<&'a mut u8>,
}

#[inline]
pub fn readv(fd: c_int, iovec: &'_ mut [IoVec<'_>]) -> Result<usize, Error> {
    unsafe { syscall!(READV, fd, iovec.as_mut_ptr(), iovec.len()) }.usize_result()
}

#[inline]
pub fn writev(fd: c_int, iovec: &'_ [IoVec<'_>]) -> Result<usize, Error> {
    unsafe { syscall!(WRITEV, fd, iovec.as_ptr(), iovec.len()) }.usize_result()
}

bitflags::bitflags! {
    pub struct Mode: c_int {
        const F_OK = 0;
        const R_OK = 4;
        const W_OK = 2;
        const X_OK = 1;
    }
}

bitflags::bitflags! {
    pub struct Pipe2Flags: c_int {
        const CLOEXEC = libc::O_CLOEXEC;
        const DIRECT = libc::O_DIRECT;
        const NONBLOCK = libc::O_NONBLOCK;
    }
}

#[inline]
pub fn pipe2(flags: Pipe2Flags) -> Result<[c_int; 2], Error> {
    let mut pipefd: [c_int; 2] = [0, 0];
    unsafe { syscall!(PIPE2, pipefd.as_mut_ptr(), flags.bits()) }.to_result_with(pipefd)
}

#[inline]
pub fn sched_yield() -> Result<(), Error> {
    unsafe { syscall!(SCHED_YIELD) }.null_result()
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

bitflags::bitflags! {
    pub struct MSync: c_int {
        const ASYNC = libc::MS_ASYNC;
        const SYNC = libc::MS_SYNC;
        const INVALIDATE = libc::MS_INVALIDATE;
    }
}

#[inline]
pub fn msync(memory: &[u8], flags: MSync) -> Result<(), Error> {
    unsafe { syscall!(MSYNC, memory.as_ptr(), memory.len(), flags.bits()) }.null_result()
}

#[inline]
pub fn mincore(memory: &[u8], status: &mut [u8]) -> Result<(), Error> {
    if status.len() < memory.len().div_ceil(4096) {
        return Err(Error(libc::EINVAL));
    }
    unsafe { syscall!(MINCORE, memory.as_ptr(), memory.len(), status.as_mut_ptr()) }.null_result()
}

bitflags::bitflags! {
    pub struct Advice: c_int {
        const NORMAL = libc::MADV_NORMAL;
        const RANDOM = libc::MADV_RANDOM;
        const SEQUENTIAL = libc::MADV_SEQUENTIAL;
        const WILLNEED = libc::MADV_WILLNEED;
        const DONTNEED = libc::MADV_DONTNEED;
        const REMOVE = libc::MADV_REMOVE;
        const DONTFORM = libc::MADV_DONTFORK;
        const DOFORK = libc::MADV_DOFORK;
        const HWPOISON = libc::MADV_HWPOISON;
        const MERGEABLE = libc::MADV_MERGEABLE;
        const UNMERGEABLE = libc::MADV_UNMERGEABLE;
        const SOFT_OFFLINE = libc::MADV_SOFT_OFFLINE;
        const HUGEPAGE = libc::MADV_HUGEPAGE;
        const NOHUGEPAGE = libc::MADV_NOHUGEPAGE;
        const DONTDUMP = libc::MADV_DONTDUMP;
        const DODUMP = libc::MADV_DODUMP;
        const FREE = libc::MADV_FREE;
        //const WIPEONFORK = libc::MADV_WIPEONFORK;
        //const KEEPONFORK = libc::MADV_KEEPONFORK;
    }
}

#[inline]
pub fn madvise(memory: &[u8], advice: Advice) -> Result<(), Error> {
    unsafe { syscall!(MADVISE, memory.as_ptr(), memory.len(), advice.bits()) }.null_result()
}

/*
bitflags::bitflags! {
    pub struct ShmFlags: c_int {
        const CREAT = libc::IPC_CREAT;
        const EXCL = libc::IPC_EXCL;
        const HUGETLB = libc::SHM_HUGETLB;
        const HUGE_2MB = libc::MAP_HUGE_2MB;
        const HUGE_1GB = libc::MAP_HUGE_1GB;
        const NORESERVE = libc::SHM_NORESERVE;
    }
}

#[inline]
pub fn shmget(key: libc::key_t, size: usize, shmflg: ShmFlags) -> Result<libc::key_t, Error> {
    unsafe { syscall!(SHMGET, key, size, shmflg.bits()) }.to_result_and(|key| key as libc::key_t)
}
*/

// shmctl
//
// dup
//
// dup2
//
// pause
//
// nanosleep
//
// getitimer
//
// alarm
//
// setitimer

#[inline]
pub fn getpid() -> libc::pid_t {
    unsafe { syscall!(GETPID) as libc::pid_t }
}

// sendfile
//
// socket
//
// connect
//
// accept
//
// sendto
//
// recvfrom
//
// sendmsg
//
// recvmsg
//
// shutdown
//
// bind
//
// listen
//
// getsockname
//
// getpeername
//
// socketpair
//
// setsockopt
//
// getsockopt

bitflags::bitflags! {
    pub struct CloneFlags: i32 {
        const VM = libc::CLONE_VM;
        const FS = libc::CLONE_FS;
        const FILES = libc::CLONE_FILES;
        const SIGHAND = libc::CLONE_SIGHAND;
        const THREAD = libc::CLONE_THREAD;
        const SYSVSEM = libc::CLONE_SYSVSEM;
        const SETTLS = libc::CLONE_SETTLS;
        const PARENT_SETTID = libc::CLONE_PARENT_SETTID;
        const CHILD_CLEARTID = libc::CLONE_CHILD_CLEARTID;
    }
}

/// # Safety
///
/// The memory pointed to by stack is given up to the thread and should be part of its own mapping
#[inline]
pub unsafe fn clone(
    flags: CloneFlags,
    stack: *mut u8,
    parent_tid: &mut libc::pid_t,
    child_tid: &mut libc::pid_t,
    tls: &mut u8,
) -> Result<libc::pid_t, Error> {
    syscall!(
        CLONE,
        flags.bits(),
        stack,
        parent_tid as *mut libc::pid_t,
        tls as *mut u8,
        child_tid as *mut libc::pid_t
    )
    .to_result_and(|v| v as libc::pid_t)
}

//
// clone
//
// fork
//
// execve

#[inline]
pub fn exit(error_code: c_int) -> ! {
    unsafe {
        syscall!(EXIT, error_code);
        core::hint::unreachable_unchecked();
    }
}

// wait4

// Require that it is non-negative
pub struct Pid(pub libc::pid_t);

pub enum SignalWhere {
    Exactly(usize),
    CurrentGroup,
    AllValid,
    Group(usize),
}

#[inline]
pub fn kill(pid: usize, signal: i32) -> Result<(), Error> {
    unsafe { syscall!(KILL, pid, signal) }.null_result()
}

// uname

pub enum FutexOp<'a> {
    Wait {
        expected: c_int,
        timeout: Option<libc::timespec>,
    },
    Wake {
        wake_at_most: c_int,
    },
    Requeue {
        wake_at_most: c_int,
        requeue_onto: &'a mut c_int,
        max_to_requeue: c_int,
    },
}

#[inline]
pub fn futex(lock: &mut c_int, op: FutexOp<'_>, private: bool) -> Result<(), Error> {
    let lock = lock as *mut c_int;
    let private = if private { libc::FUTEX_PRIVATE_FLAG } else { 0 };
    unsafe {
        match op {
            FutexOp::Wait { expected, timeout } => {
                let timeout = timeout
                    .map(|mut t| &mut t as *mut libc::timespec)
                    .unwrap_or(core::ptr::null_mut());
                syscall!(FUTEX, lock, libc::FUTEX_WAIT | private, expected, timeout)
            }
            FutexOp::Wake { wake_at_most } => {
                syscall!(FUTEX, lock, libc::FUTEX_WAKE | private, wake_at_most)
            }
            FutexOp::Requeue {
                wake_at_most,
                requeue_onto,
                max_to_requeue,
            } => syscall!(
                FUTEX,
                lock,
                libc::FUTEX_REQUEUE | private,
                wake_at_most,
                max_to_requeue,
                requeue_onto as *mut c_int
            ),
        }
    }
    .null_result()
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
pub fn getdents64(fd: c_int, buf: &mut [MaybeUninit<u8>]) -> Result<usize, Error> {
    unsafe { syscall!(GETDENTS64, fd, buf.as_mut_ptr(), buf.len()) }.to_result_and(|n| n)
}

#[inline]
pub fn faccessat(fd: c_int, name: CStr, mode: c_int) -> Result<(), Error> {
    unsafe { syscall!(FACCESSAT, fd, name.as_ptr(), mode) }.null_result()
}

#[inline]
pub fn readlinkat<'a>(fd: c_int, name: CStr, buf: &'a mut [u8]) -> Result<&'a [u8], Error> {
    match unsafe { syscall!(READLINKAT, fd, name.as_ptr(), buf.as_mut_ptr(), buf.len()) }
        .to_result_and(|n| n)
    {
        Ok(n) => Ok(buf.get(..n).unwrap_or_default()),
        Err(e) => Err(e),
    }
}

#[inline]
pub fn gettimeofday() -> Result<libc::timeval, Error> {
    let mut tv = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    unsafe { syscall!(GETTIMEOFDAY, &mut tv as *mut libc::timeval, 0) }.to_result_with(tv)
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
