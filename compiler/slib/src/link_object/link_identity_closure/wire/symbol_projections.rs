//! Compare symbol tables with the original wire without cloning candidates.

use super::*;
use scoop_wire::{WirePath, encode_canonical_temporary_with_meter};

impl DecodedLinkIdentityClosureSectionV1 {
    pub fn replay_symbol_projections(
        &self,
        defined: &CanonicalDefinedLinkSymbolOwnerSetV1,
        undefined: &CanonicalUndefinedSymbolRequirementSetV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), LinkSymbolProjectionValidationError> {
        if !same_bytes(&self.defined_symbols, defined, meter, 4)? {
            return Err(LinkSymbolProjectionValidationError::DefinedSymbols(
                DefinedLinkSymbolOwnerValidationError::ProjectionMismatch,
            ));
        }
        if !same_bytes(&self.undefined_symbols, undefined, meter, 5)? {
            return Err(LinkSymbolProjectionValidationError::UndefinedSymbols(
                UndefinedSymbolRequirementValidationError::ProjectionMismatch,
            ));
        }
        Ok(())
    }
}

fn same_bytes(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    meter: &mut BudgetMeter,
    field: u32,
) -> Result<bool, LinkSymbolProjectionValidationError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    Ok(actual == expected)
}

impl From<WireError> for LinkSymbolProjectionValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
