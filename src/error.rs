pub struct Error(libc::c_int);

impl Error {
    pub fn last_os_error() -> Self {
        Error(unsafe { *libc::__errno_location() })
    }
}

impl<T> From<arrayvec::CapacityError<T>> for Error {
    fn from(e: arrayvec::CapacityError<T>) -> Error {
        Error(0)
    }
}
