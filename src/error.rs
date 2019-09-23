pub struct Error(pub isize);

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
