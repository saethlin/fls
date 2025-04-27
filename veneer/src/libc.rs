#![allow(non_camel_case_types)]
use core::ffi::{c_int, c_long, c_short, c_ulong, c_ushort};

// fs/io
pub const AT_FDCWD: c_int = -100;

pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;

pub const EBADF: c_int = 9;
pub const EAGAIN: c_int = 11;
pub const EINVAL: c_int = 22;

pub const O_RDONLY: c_int = 0;
pub const O_WRONLY: c_int = 1;
pub const O_RDWR: c_int = 2;
pub const O_CREAT: c_int = 64;
pub const O_EXCL: c_int = 128;
pub const O_NOCTTY: c_int = 256;
pub const O_TRUNC: c_int = 512;
pub const O_APPEND: c_int = 1024;
pub const O_NONBLOCK: c_int = 2048;
pub const O_DSYNC: c_int = 4096;
pub const O_SYNC: c_int = 1052672;
pub const O_RSYNC: c_int = 1052672;
pub const O_FSYNC: c_int = 0x101000;
pub const O_NOATIME: c_int = 0o1000000;
pub const O_PATH: c_int = 0o10000000;
pub const O_TMPFILE: c_int = 0o20000000 | O_DIRECTORY;

pub const O_ASYNC: c_int = 0x2000;
pub const O_NDELAY: c_int = 0x800;

pub const O_CLOEXEC: c_int = 0x80000;
pub const O_DIRECT: c_int = 0x4000;
pub const O_DIRECTORY: c_int = 0x10000;
pub const O_LARGEFILE: c_int = 0;
pub const O_NOFOLLOW: c_int = 0x20000;

pub type mode_t = u32;

pub const S_IFIFO: mode_t = 0o1_0000;
pub const S_IFCHR: mode_t = 0o2_0000;
pub const S_IFBLK: mode_t = 0o6_0000;
pub const S_IFDIR: mode_t = 0o4_0000;
pub const S_IFREG: mode_t = 0o10_0000;
pub const S_IFLNK: mode_t = 0o12_0000;
pub const S_IFSOCK: mode_t = 0o14_0000;
pub const S_IFMT: mode_t = 0o17_0000;
pub const S_IRWXU: mode_t = 0o0700;
pub const S_IXUSR: mode_t = 0o0100;
pub const S_IWUSR: mode_t = 0o0200;
pub const S_IRUSR: mode_t = 0o0400;
pub const S_IRWXG: mode_t = 0o0070;
pub const S_IXGRP: mode_t = 0o0010;
pub const S_IWGRP: mode_t = 0o0020;
pub const S_IRGRP: mode_t = 0o0040;
pub const S_IRWXO: mode_t = 0o0007;
pub const S_IXOTH: mode_t = 0o0001;
pub const S_IWOTH: mode_t = 0o0002;
pub const S_IROTH: mode_t = 0o0004;
pub const S_ISUID: mode_t = 0o4000;
pub const S_ISGID: mode_t = 0o2000;
pub const S_ISVTX: mode_t = 0o1000;

pub const F_OK: c_int = 0;
pub const R_OK: c_int = 4;
pub const W_OK: c_int = 2;
pub const X_OK: c_int = 1;

pub type dev_t = u64;
pub type ino_t = u64;
pub type nlink_t = u64;
pub type uid_t = u32;
pub type gid_t = u32;
pub type off_t = i64;
pub type blksize_t = i64;
pub type blkcnt_t = i64;
pub type time_t = i64;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct stat {
    pub st_dev: dev_t,
    pub st_ino: ino_t,
    pub st_nlink: nlink_t,
    pub st_mode: mode_t,
    pub st_uid: uid_t,
    pub st_gid: gid_t,
    __pad0: c_int,
    pub st_rdev: dev_t,
    pub st_size: off_t,
    pub st_blksize: blksize_t,
    pub st_blocks: blkcnt_t,
    pub st_atime: time_t,
    pub st_atime_nsec: i64,
    pub st_mtime: time_t,
    pub st_mtime_nsec: i64,
    pub st_ctime: time_t,
    pub st_ctime_nsec: i64,
    __unused: [i64; 3],
}

pub type blkcnt64_t = i64;
pub type ino64_t = u64;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct stat64 {
    pub st_dev: dev_t,
    pub st_ino: ino64_t,
    pub st_nlink: nlink_t,
    pub st_mode: mode_t,
    pub st_uid: uid_t,
    pub st_gid: gid_t,
    __pad0: c_int,
    pub st_rdev: dev_t,
    pub st_size: off_t,
    pub st_blksize: blksize_t,
    pub st_blocks: blkcnt64_t,
    pub st_atime: time_t,
    pub st_atime_nsec: i64,
    pub st_mtime: time_t,
    pub st_mtime_nsec: i64,
    pub st_ctime: time_t,
    pub st_ctime_nsec: i64,
    __reserved: [i64; 3],
}

pub const AT_SYMLINK_NOFOLLOW: c_int = 0x100;

pub type suseconds_t = i64;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct timeval {
    pub tv_sec: time_t,
    pub tv_usec: suseconds_t,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct pollfd {
    pub fd: c_int,
    pub events: c_short,
    pub revents: c_short,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct timespec {
    pub tv_sec: time_t,
    pub tv_nsec: c_long,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct sigset_t {
    #[cfg(target_pointer_width = "32")]
    __val: [u32; 32],
    #[cfg(target_pointer_width = "64")]
    __val: [u64; 16],
}

pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;

// mmap
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;

pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_ANON: c_int = 0x0020;

pub const MREMAP_MAYMOVE: c_int = 1;

pub const MADV_NORMAL: c_int = 0;
pub const MADV_RANDOM: c_int = 1;
pub const MADV_SEQUENTIAL: c_int = 2;
pub const MADV_WILLNEED: c_int = 3;
pub const MADV_DONTNEED: c_int = 4;
pub const MADV_FREE: c_int = 8;
pub const MADV_REMOVE: c_int = 9;
pub const MADV_DONTFORK: c_int = 10;
pub const MADV_DOFORK: c_int = 11;
pub const MADV_MERGEABLE: c_int = 12;
pub const MADV_UNMERGEABLE: c_int = 13;
pub const MADV_HUGEPAGE: c_int = 14;
pub const MADV_NOHUGEPAGE: c_int = 15;
pub const MADV_DONTDUMP: c_int = 16;
pub const MADV_DODUMP: c_int = 17;
pub const MADV_WIPEONFORK: c_int = 18;
pub const MADV_KEEPONFORK: c_int = 19;
pub const MADV_COLD: c_int = 20;
pub const MADV_PAGEOUT: c_int = 21;
pub const MADV_HWPOISON: c_int = 100;
pub const MADV_SOFT_OFFLINE: c_int = 101;

pub const MS_ASYNC: c_int = 0x0001;
pub const MS_INVALIDATE: c_int = 0x0002;
pub const MS_SYNC: c_int = 0x0004;

pub type pid_t = i32;

// threads
pub const CLONE_VM: c_int = 0x100;
pub const CLONE_FS: c_int = 0x200;
pub const CLONE_FILES: c_int = 0x400;
pub const CLONE_SIGHAND: c_int = 0x800;
pub const CLONE_PTRACE: c_int = 0x2000;
pub const CLONE_VFORK: c_int = 0x4000;
pub const CLONE_PARENT: c_int = 0x8000;
pub const CLONE_THREAD: c_int = 0x10000;
pub const CLONE_NEWNS: c_int = 0x20000;
pub const CLONE_SYSVSEM: c_int = 0x40000;
pub const CLONE_SETTLS: c_int = 0x80000;
pub const CLONE_PARENT_SETTID: c_int = 0x100000;
pub const CLONE_CHILD_CLEARTID: c_int = 0x200000;

// futex
pub const FUTEX_WAIT: c_int = 0;
pub const FUTEX_WAKE: c_int = 1;
pub const FUTEX_REQUEUE: c_int = 3;
pub const FUTEX_PRIVATE_FLAG: c_int = 128;

// winsize
pub const TIOCGWINSZ: c_ulong = 0x5413;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct winsize {
    pub ws_row: c_ushort,
    pub ws_col: c_ushort,
    pub ws_xpixel: c_ushort,
    pub ws_ypixel: c_ushort,
}
