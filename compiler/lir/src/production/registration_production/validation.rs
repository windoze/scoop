//! Validation of untrusted complete registration-production carriers.

use std::collections::BTreeSet;
use std::fmt;
use std::num::NonZeroU64;

use scoop_identity::{
    DecodedPersistentId, DefinitionAtomRole, LinkageClass, PersistentId, PersistentStaticStorageId,
    PersistentSymbolKey, PersistentSymbolRequest, RepresentationRole, ScanRole, StaticStorageKey,
};
use scoop_wire::{WireEncode, encode};

use super::wire::{
    DecodedImmortalObjectTypeRegistrationRefV1, DecodedOptionalStrongTypeDescriptorRefV1,
    DecodedRefScan, DecodedStaticImmortalRelocationPlanV1, DecodedStrongCallableRuntimeScanPlanV1,
    DecodedStrongInitializationSchedulePlanV1, DecodedStrongStaticStorageInitialStatePlanV1,
    DecodedStrongTypeDescriptorRefV1, DecodedStrongTypeDispatchCallableRefV1,
    DecodedStrongTypeRegistrationPlanV1, DecodedTypeDescriptorInlineScanV1,
    DecodedTypeInstanceShapeV1,
};
use super::{
    DecodedStrongImmortalObjectRegistrationPlanV1,
    DecodedStrongInitializationUnitRegistrationPlanV1,
    DecodedStrongRegistrationProductionSurfaceV1, DecodedStrongSafepointRegistrationPlanV1,
    DecodedStrongStaticStorageRegistrationPlanV1, StrongRegistrationProductionBuildError,
    StrongRegistrationProductionSurfaceV1,
};
use crate::{
    ArrayElementStorageV1, BackendScalarKind, ImmortalObjectTypeRegistrationRefV1,
    LirTargetProfile, NonEmptyRefScan, OdrFreeLirFoundation, PointerKind, RefScan, RuntimeFunction,
    StaticImmortalRelocationPlanV1, StaticStorageScanKindV1, StrongCallableRuntimeScanAtomV1,
    StrongCallableRuntimeScanPlanSetV1, StrongCallableRuntimeScanPlanV1,
    StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeV1,
    StrongImmortalObjectSemanticPlanSetV1, StrongImmortalObjectSemanticPlanV1,
    StrongInitializationSchedulePlanV1, StrongInitializationUnitSemanticPlanSetV1,
    StrongInitializationUnitSemanticPlanV1, StrongRegistrationIdentitySurfaceV1,
    StrongRegistrationIdentityValidationError, StrongSafepointSemanticPlanSetV1,
    StrongSafepointSemanticPlanV1, StrongStaticStorageInitialStatePlanV1,
    StrongStaticStorageSemanticPlanSetV1, StrongStaticStorageSemanticPlanV1,
    StrongTypeDescriptorRefV1, StrongTypeDescriptorSemanticPlanSetV1,
    StrongTypeDispatchCallableRefV1, TypeDescriptorInlineScanV1, TypeInstanceKindV1,
    TypeInstanceShapeV1, ValueStorageLayoutV1, generated_unit_body, startup_gateway_body,
};

mod callables;
use callables::{validate_callable_runtime_scans, validate_safepoints};
mod types;
pub use types::validate_type_registration_constituents_v2;
pub(crate) use types::validate_types;
mod type_shape;
use type_shape::{validate_type_descriptor_inline_scan, validate_type_instance_shape};
mod type_references;

mod immortal;
use immortal::validate_immortal_objects;
mod static_storage;
use static_storage::validate_static_storages;
mod initialization;
use initialization::validate_initialization_units;

impl DecodedStrongRegistrationProductionSurfaceV1 {
    pub fn validate(
        self,
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        digests: &StrongDigestFinalizationPlanV1,
        external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<StrongRegistrationProductionSurfaceV1, StrongRegistrationProductionValidationError>
    {
        let actual = encode(&self).map_err(StrongRegistrationProductionValidationError::Encode)?;
        let identities = self
            .identities
            .validate(foundation, digests)
            .map_err(StrongRegistrationProductionValidationError::Identities)?;
        let safepoint_semantics = validate_safepoints(self.safepoints, foundation, &identities)?;
        let callable_runtime_scans =
            validate_callable_runtime_scans(self.callable_runtime_scans, foundation)?;
        validate_derived_table(
            RegistrationProductionTableV1::Callable,
            &self.callables,
            crate::StrongCallableRegistrationPlanSetV1::new(
                foundation,
                &identities,
                callable_runtime_scans.clone(),
                digests,
            )
            .map_err(|error| {
                StrongRegistrationProductionValidationError::Expected(Box::new(
                    StrongRegistrationProductionBuildError::Callables(error),
                ))
            })?
            .registrations(),
        )?;
        let type_semantics = validate_types(
            self.types,
            target,
            foundation,
            &identities,
            external_bridges,
            digests,
        )?;
        let immortal_semantics = validate_immortal_objects(
            self.immortal_objects,
            target,
            foundation,
            &identities,
            external_bridges,
        )?;
        let static_semantics =
            validate_static_storages(self.static_storages, target, foundation, &identities)?;
        let initialization_semantics = validate_initialization_units(
            self.initialization_units,
            target,
            foundation,
            &identities,
            static_semantics,
        )?;
        let expected = StrongRegistrationProductionSurfaceV1::from_semantics(
            target,
            foundation,
            digests,
            identities,
            callable_runtime_scans,
            type_semantics,
            safepoint_semantics,
            immortal_semantics,
            initialization_semantics,
        )
        .map_err(|error| StrongRegistrationProductionValidationError::Expected(Box::new(error)))?;
        let expected_bytes =
            encode(&expected).map_err(StrongRegistrationProductionValidationError::Encode)?;
        if actual != expected_bytes {
            return Err(StrongRegistrationProductionValidationError::SurfaceMismatch);
        }
        Ok(expected)
    }
}

fn validate_derived_table<A: WireEncode, E: WireEncode>(
    table: RegistrationProductionTableV1,
    actual: &[A],
    expected: &[E],
) -> Result<(), StrongRegistrationProductionValidationError> {
    require_length(table, actual.len(), expected.len())?;
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let actual = encode(actual).map_err(StrongRegistrationProductionValidationError::Encode)?;
        let expected =
            encode(expected).map_err(StrongRegistrationProductionValidationError::Encode)?;
        if actual != expected {
            return Err(StrongRegistrationProductionValidationError::EntryMismatch {
                table,
                index,
            });
        }
    }
    Ok(())
}

fn require_length(
    table: RegistrationProductionTableV1,
    actual: usize,
    expected: usize,
) -> Result<(), StrongRegistrationProductionValidationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(StrongRegistrationProductionValidationError::TableLength {
            table,
            expected,
            actual,
        })
    }
}

fn verify_expected<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> Result<I, StrongRegistrationProductionValidationError> {
    decoded
        .verify(expected)
        .map_err(|_| semantic_error(table, index, field))
}

fn resolve_known<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    candidates: impl IntoIterator<Item = I>,
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> Result<I, StrongRegistrationProductionValidationError> {
    candidates
        .into_iter()
        .find(|candidate| candidate.as_array() == decoded.as_array())
        .ok_or_else(|| semantic_error(table, index, field))
}

fn semantic_error(
    table: RegistrationProductionTableV1,
    index: usize,
    field: &'static str,
) -> StrongRegistrationProductionValidationError {
    StrongRegistrationProductionValidationError::Semantic {
        table,
        index,
        field,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationProductionTableV1 {
    Safepoint,
    Callable,
    Type,
    ImmortalObject,
    StaticStorage,
    InitializationUnit,
}

#[derive(Debug)]
pub enum StrongRegistrationProductionValidationError {
    Encode(scoop_wire::cbor::EncodeError),
    Resource(scoop_wire::WireError),
    TypeReference(crate::StrongTypeReferenceResolutionErrorV2),
    Identities(StrongRegistrationIdentityValidationError),
    TableLength {
        table: RegistrationProductionTableV1,
        expected: usize,
        actual: usize,
    },
    EntryMismatch {
        table: RegistrationProductionTableV1,
        index: usize,
    },
    Semantic {
        table: RegistrationProductionTableV1,
        index: usize,
        field: &'static str,
    },
    Expected(Box<StrongRegistrationProductionBuildError>),
    SurfaceMismatch,
}

impl fmt::Display for StrongRegistrationProductionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid decoded strong registration production surface: {self:?}"
        )
    }
}

impl std::error::Error for StrongRegistrationProductionValidationError {}

impl From<scoop_wire::WireError> for StrongRegistrationProductionValidationError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<crate::StrongTypeReferenceResolutionErrorV2>
    for StrongRegistrationProductionValidationError
{
    fn from(error: crate::StrongTypeReferenceResolutionErrorV2) -> Self {
        Self::TypeReference(error)
    }
}
