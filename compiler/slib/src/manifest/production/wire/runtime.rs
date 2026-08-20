//! Compare the original runtime fields with actual finalized registrations.

use super::*;
use crate::VerifiedRuntimeImageFingerprintV2;

impl DecodedSingleConeProductionManifestV1 {
    pub fn replay_runtime_projection(
        &self,
        identities: &scoop_lir::RegistrationIdentitySurfaceV1,
        image: &VerifiedRuntimeImageFingerprintV2,
    ) -> Result<(), RuntimeProductionProjectionError> {
        same(&self.image_owner_member, &image.image().member(), 3)?;
        same(&self.runtime_registration_projection, identities, 4)?;
        same(&self.runtime_image_fingerprint, &image.fingerprint(), 6)
    }
}

fn same(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    field: u32,
) -> Result<(), RuntimeProductionProjectionError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary(actual, &path)?;
    let expected = encode_canonical_temporary(expected, &path)?;

    if actual != expected {
        return Err(RuntimeProductionProjectionError::FieldMismatch { field });
    }
    Ok(())
}

#[derive(Debug)]
pub enum RuntimeProductionProjectionError {
    Resource(WireError),
    FieldMismatch { field: u32 },
}

impl From<WireError> for RuntimeProductionProjectionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for RuntimeProductionProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid runtime production projection: {self:?}")
    }
}

impl std::error::Error for RuntimeProductionProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::FieldMismatch { .. } => None,
        }
    }
}
