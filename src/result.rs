use std::{fmt, io, num, str};

#[derive(Debug)]
pub enum DictError {
    Io(io::Error),
    Utf8(str::Utf8Error),
    Parse(num::ParseIntError),
    My(String),
}

impl From<io::Error> for DictError {
    fn from(err: io::Error) -> Self {
        DictError::Io(err)
    }
}
impl From<str::Utf8Error> for DictError {
    fn from(err: str::Utf8Error) -> Self {
        DictError::Utf8(err)
    }
}
impl From<num::ParseIntError> for DictError {
    fn from(err: num::ParseIntError) -> Self {
        DictError::Parse(err)
    }
}
impl fmt::Display for DictError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            DictError::Io(err) => write!(f, "{}", err),
            DictError::Utf8(err) => write!(f, "{}", err),
            DictError::Parse(err) => write!(f, "{}", err),
            DictError::My(msg) => write!(f, "{}", msg),
        }
    }
}
