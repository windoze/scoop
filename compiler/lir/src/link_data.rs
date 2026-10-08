//! Machine-data decoding shared by artifact consumers, without a source world.

use std::fmt;

pub(crate) mod value_storage;

#[derive(Debug)]
pub struct LinkDataError(pub String);

impl fmt::Display for LinkDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LinkDataError {}

pub(crate) fn link_error(error: impl fmt::Display) -> LinkDataError {
    LinkDataError(error.to_string())
}
