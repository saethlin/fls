pub struct Error(pub i32);

impl<T> From<arrayvec::CapacityError<T>> for Error {
    fn from(_e: arrayvec::CapacityError<T>) -> Error {
        Error(0)
    }
}

impl From<i32> for Error {
    fn from(e: i32) -> Error {
        Error(e)
    }
}
