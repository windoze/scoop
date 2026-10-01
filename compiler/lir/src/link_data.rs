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

pub(crate) fn same_wire(
    actual: &impl scoop_wire::WireEncode,
    expected: &impl scoop_wire::WireEncode,
    role: &str,
) -> Result<(), LinkDataError> {
    if scoop_wire::encode(actual).map_err(link_error)?
        != scoop_wire::encode(expected).map_err(link_error)?
    {
        return Err(LinkDataError(format!("noncanonical {role} machine data")));
    }
    Ok(())
}
