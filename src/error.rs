pub struct Error(pub isize);

impl<T> From<arrayvec::CapacityError<T>> for Error {
    fn from(_e: arrayvec::CapacityError<T>) -> Error {
        Error(0)
    }
}

impl From<i32> for Error {
    fn from(e: i32) -> Error {
        Error(e as isize)
    }
}

impl From<isize> for Error {
    fn from(e: isize) -> Error {
        Error(e)
    }
}
