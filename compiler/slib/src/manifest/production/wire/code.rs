//! Compare the remaining manifest fields with independently rebuilt Code inputs.

use super::*;
use crate::{CanonicalNativeExternalContractCodeSetV1, SingleConeProductionCodeProjectionV1};
use scoop_lir::CanonicalNativeLibraryRequirementV1;

impl DecodedSingleConeProductionManifestV1 {
    pub(crate) fn replay_code_projection(
        &self,
        production: &SingleConeProductionCodeProjectionV1,
        code: CodeFingerprint,
        contracts: &CanonicalNativeExternalContractCodeSetV1,
        libraries: &[CanonicalNativeLibraryRequirementV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), CodeProductionProjectionError> {
        same(&self.distribution, &production.distribution(), 1, meter)?;
        same(&self.output, production.output(), 2, meter)?;
        same(&self.code_fingerprint, &code, 7, meter)?;
        same(&self.native_contracts, contracts, 8, meter)?;
        same(
            &Array(&self.native_library_requirements),
            &Array(libraries),
            9,
            meter,
        )
    }
}

struct Array<'a, T>(&'a [T]);
impl<T: WireEncode> WireEncode for Array<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, self.0)
    }
}

fn same(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    field: u32,
    meter: &mut BudgetMeter,
) -> Result<(), CodeProductionProjectionError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    if actual != expected {
        return Err(CodeProductionProjectionError::FieldMismatch { field });
    }
    Ok(())
}

#[derive(Debug)]
pub enum CodeProductionProjectionError {
    Resource(WireError),
    FieldMismatch { field: u32 },
}

impl From<WireError> for CodeProductionProjectionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for CodeProductionProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid Code production projection: {self:?}")
    }
}

impl std::error::Error for CodeProductionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::FieldMismatch { .. } => None,
        }
    }
}
