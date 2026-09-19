use scoop_wire::{BudgetMeter, WireError, WirePath};

#[cfg(test)]
mod tests;
use std::fmt;

/// Preserves the original constituent error while adding shared resource
/// accounting for a new aggregate reader. Existing wire remains unchanged.
#[derive(Debug)]
pub enum MeteredInterfaceResolutionError<T> {
    Resource(WireError),
    Value(T),
}
impl<T: fmt::Display> fmt::Display for MeteredInterfaceResolutionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
        }
    }
}
impl<T: std::error::Error + 'static> std::error::Error for MeteredInterfaceResolutionError<T> {}

pub(super) fn charge_name<T>(
    name: &scoop_identity::DecodedCanonicalIdentifier,
    copies: u64,
    meter: &mut BudgetMeter,
) -> Result<(), MeteredInterfaceResolutionError<T>> {
    let length = name.byte_len() as u64;
    let path = WirePath::root();
    meter
        .charge_owned_bytes(length.saturating_mul(copies), &path)
        .map_err(MeteredInterfaceResolutionError::Resource)?;
    meter
        .charge_work(length, &path)
        .map_err(MeteredInterfaceResolutionError::Resource)
}
