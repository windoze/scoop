//! Compare the original runtime fields with actual finalized registrations.

use super::*;
use crate::{CanonicalStrongRegistrationFingerprintSetV1, VerifiedRuntimeImageFingerprintV2};

impl DecodedSingleConeProductionManifestV1 {
    pub fn replay_runtime_projection(
        &self,
        identities: &scoop_lir::StrongRegistrationIdentitySurfaceV1,
        image: &VerifiedRuntimeImageFingerprintV2,
        meter: &mut BudgetMeter,
    ) -> Result<(), RuntimeProductionProjectionError> {
        let registrations = image.registrations();
        let count = [
            registrations.safepoints().fingerprints().len(),
            registrations.callables().fingerprints().len(),
            registrations.types().fingerprints().len(),
            registrations.immortal_objects().fingerprints().len(),
            registrations.static_storages().fingerprints().len(),
            registrations.initializations().fingerprints().len(),
        ]
        .into_iter()
        .map(|count| count as u64)
        .sum::<u64>();
        let path = WirePath::root().field(5);
        meter.check_table_entries(count, &path)?;
        meter.charge_collection_slots(count, &path)?;
        meter.charge_owned_bytes(count.saturating_mul(64), &path)?;
        meter.charge_work(
            count.saturating_mul(u64::from(count.max(1).ilog2()) + 1),
            &path,
        )?;
        let set = CanonicalStrongRegistrationFingerprintSetV1::from_patch_set(registrations)
            .map_err(RuntimeProductionProjectionError::RegistrationSet)?;
        same(&self.image_owner_member, &image.image().member(), 3, meter)?;
        same(&self.runtime_registration_projection, identities, 4, meter)?;
        same(&self.strong_registration_set, &set, 5, meter)?;
        same(
            &self.runtime_image_fingerprint,
            &image.fingerprint(),
            6,
            meter,
        )
    }
}

fn same(
    actual: &impl WireEncode,
    expected: &impl WireEncode,
    field: u32,
    meter: &mut BudgetMeter,
) -> Result<(), RuntimeProductionProjectionError> {
    let path = WirePath::root().field(field);
    let actual = encode_canonical_temporary_with_meter(actual, meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(expected, meter, &path)?;
    meter.charge_work(actual.len().min(expected.len()) as u64, &path)?;
    if actual != expected {
        return Err(RuntimeProductionProjectionError::FieldMismatch { field });
    }
    Ok(())
}

#[derive(Debug)]
pub enum RuntimeProductionProjectionError {
    Resource(WireError),
    FieldMismatch { field: u32 },
    RegistrationSet(crate::StrongRegistrationFingerprintProjectionError),
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
            Self::RegistrationSet(error) => Some(error),
            Self::FieldMismatch { .. } => None,
        }
    }
}
