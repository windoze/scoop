//! Equality replay for typed semantic projections from checked provider plans.
use scoop_wire::{
    BudgetMeter, WireEncode, WireError, WirePath, encode_canonical_temporary_with_meter,
};

#[derive(Debug)]
pub enum StrongSemanticProjectionError {
    Mismatch,
    Resource(WireError),
}
impl From<WireError> for StrongSemanticProjectionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for StrongSemanticProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Strong semantic projection differs from checked plan: {self:?}"
        )
    }
}
impl std::error::Error for StrongSemanticProjectionError {}

pub(super) fn compare_semantics(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    meter: &mut BudgetMeter,
) -> Result<(), StrongSemanticProjectionError> {
    let path = WirePath::root();
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len() as u64, &path)?;
    if actual != expected {
        return Err(StrongSemanticProjectionError::Mismatch);
    }
    Ok(())
}
