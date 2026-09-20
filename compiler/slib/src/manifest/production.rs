//! Closed production-manifest projection for the M23-3 strong-only profile.

use std::fmt;

use scoop_identity::{
    DigestNodeId, MainCallableBodyId, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentStaticStorageId, SourceSignatureFingerprint,
};
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeLibraryRequirementV1, EntryProductionPlanV1,
    StrongProductionSection, StrongProductionSectionV1, StrongRegistrationIdentitySurfaceV1,
    StrongRegistrationIdentityV1,
};
use scoop_wire::{Encoder, WireEncode};

use super::{ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, RuntimeImageFingerprint};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalStrongRegistrationFingerprintSetV1, ObjectDefinitionFingerprintV1,
    StrongRegistrationFingerprintProjectionError, VerifiedCodeFingerprintV1,
    VerifiedCodeLinkObjectMemberSetV1, VerifiedEntryProductionBranchV1,
    VerifiedStrongRegistrationPatchSetV1,
};

mod wire;
pub use wire::{
    CBridgeCheckedSingleConeProductionManifestV1, DecodedSingleConeProductionManifestV1,
    SingleConeProductionManifestValidationError,
};

#[cfg(test)]
pub(crate) fn encoded_library_production_manifest_for_test() -> Vec<u8> {
    wire::tests::library_manifest_bytes()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactDistributionClassV1 {
    DistributableCone,
    LocalExecutableRoot,
}

impl WireEncode for ArtifactDistributionClassV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::DistributableCone => 1,
            Self::LocalExecutableRoot => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableRootProjectionV1 {
    main: MainCallableBodyId,
    source_signature_fingerprint: SourceSignatureFingerprint,
    gateway: PersistentCallableBodyId,
    gateway_definition_fingerprint: ObjectDefinitionFingerprintV1,
    failure_root: PersistentStaticStorageId,
    entry_owner_member: SlibMemberId,
}

impl ExecutableRootProjectionV1 {
    pub const fn main(self) -> MainCallableBodyId {
        self.main
    }

    pub const fn source_signature_fingerprint(self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }

    pub const fn gateway(self) -> PersistentCallableBodyId {
        self.gateway
    }

    pub const fn gateway_definition_fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.gateway_definition_fingerprint
    }

    pub const fn failure_root(self) -> PersistentStaticStorageId {
        self.failure_root
    }

    pub const fn entry_owner_member(self) -> SlibMemberId {
        self.entry_owner_member
    }
}

impl WireEncode for ExecutableRootProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.main.encode(encoder)?;
        encoder.field(2)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.gateway.encode(encoder)?;
        encoder.field(4)?;
        self.gateway_definition_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.failure_root.encode(encoder)?;
        encoder.field(6)?;
        self.entry_owner_member.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SingleConeProductionOutputV1 {
    Library,
    Executable(Box<ExecutableRootProjectionV1>),
}

impl WireEncode for SingleConeProductionOutputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(root) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                root.encode(encoder)
            }
        }
    }
}

/// The manifest contribution to `CodeFingerprint`; it cannot contain that
/// fingerprint and therefore cannot form a self-reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleConeProductionCodeProjectionV1 {
    distribution: ArtifactDistributionClassV1,
    output: SingleConeProductionOutputV1,
    image_owner_member: SlibMemberId,
    runtime_registration_projection: StrongRegistrationIdentitySurfaceV1,
    strong_registration_set: CanonicalStrongRegistrationFingerprintSetV1,
    runtime_image_fingerprint: RuntimeImageFingerprint,
}

impl SingleConeProductionCodeProjectionV1 {
    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.distribution
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        &self.output
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.image_owner_member
    }

    pub const fn runtime_registration_projection(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        &self.runtime_registration_projection
    }

    pub const fn strong_registration_set(&self) -> &CanonicalStrongRegistrationFingerprintSetV1 {
        &self.strong_registration_set
    }

    pub const fn runtime_image_fingerprint(&self) -> RuntimeImageFingerprint {
        self.runtime_image_fingerprint
    }
}

impl WireEncode for SingleConeProductionCodeProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.distribution.encode(encoder)?;
        encoder.field(2)?;
        self.output.encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member.encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection.encode(encoder)?;
        encoder.field(5)?;
        self.strong_registration_set.encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint.encode(encoder)
    }
}

/// Strong LIR metadata and finalized object bytes proven to describe the same
/// code-sink manifest projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSingleConeProductionCodeProjectionV1 {
    strong_production: StrongProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
    projection: SingleConeProductionCodeProjectionV1,
}

impl VerifiedSingleConeProductionCodeProjectionV1 {
    pub const fn strong_production(&self) -> &StrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn link_objects(&self) -> &VerifiedCodeLinkObjectMemberSetV1 {
        &self.link_objects
    }

    pub const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        &self.projection
    }
}

/// The complete ten-field manifest payload. All fields remain derived from
/// the owned code proof, so repeated digests and projections cannot diverge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleConeProductionManifestV1 {
    code: VerifiedCodeFingerprintV1,
}

impl SingleConeProductionManifestV1 {
    pub const fn from_verified_code(code: VerifiedCodeFingerprintV1) -> Self {
        Self { code }
    }

    pub const fn code_proof(&self) -> &VerifiedCodeFingerprintV1 {
        &self.code
    }

    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.projection().distribution()
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        self.projection().output()
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.projection().image_owner_member()
    }

    pub const fn runtime_registration_projection(&self) -> &StrongRegistrationIdentitySurfaceV1 {
        self.projection().runtime_registration_projection()
    }

    pub const fn strong_registration_set(&self) -> &CanonicalStrongRegistrationFingerprintSetV1 {
        self.projection().strong_registration_set()
    }

    pub const fn runtime_image_fingerprint(&self) -> RuntimeImageFingerprint {
        self.projection().runtime_image_fingerprint()
    }

    pub const fn code_fingerprint(&self) -> super::CodeFingerprint {
        self.code.fingerprint()
    }

    pub const fn native_contracts(
        &self,
    ) -> &crate::link_object::CanonicalNativeExternalContractCodeSetV1 {
        self.code.native_contracts()
    }

    pub fn native_library_requirements(&self) -> &[CanonicalNativeLibraryRequirementV1] {
        self.code.native_requirements().library_requirements()
    }

    pub const fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        self.code.c_bridge_production()
    }

    const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        self.code.production().projection()
    }
}

impl WireEncode for SingleConeProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.distribution().encode(encoder)?;
        encoder.field(2)?;
        self.output().encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member().encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection().encode(encoder)?;
        encoder.field(5)?;
        self.strong_registration_set().encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint().encode(encoder)?;
        encoder.field(7)?;
        self.code_fingerprint().encode(encoder)?;
        encoder.field(8)?;
        self.native_contracts().encode(encoder)?;
        encoder.field(9)?;
        encode_array(encoder, self.native_library_requirements())?;
        encoder.field(10)?;
        self.c_bridge_production().encode(encoder)
    }
}

pub fn verify_single_cone_production_code_projection_v1(
    cone: &ConeRecord,
    direct_dependencies: &[DependencyRecord],
    source_count: usize,
    strong_production: StrongProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let dependency_identities = direct_dependencies
        .iter()
        .map(DependencyRecord::identity)
        .collect::<Vec<_>>();
    verify_production_code_projection_v1(
        cone,
        &dependency_identities,
        &dependency_identities,
        source_count,
        strong_production,
        link_objects,
    )
}

/// Verifies the M23-5 production projection without changing the frozen
/// M23-3 image-plan dependency contract. Ordinary direct dependencies belong
/// to the artifact graph and semantic/link closures; the legacy runtime image
/// descriptor continues to name only the implicit trusted-core dependency.
pub fn verify_cross_cone_production_code_projection_v1(
    cone: &ConeRecord,
    direct_dependencies: &[DependencyRecord],
    source_count: usize,
    strong_production: StrongProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let dependency_identities = direct_dependencies
        .iter()
        .map(DependencyRecord::identity)
        .collect::<Vec<_>>();
    let legacy_image_dependencies = if cone.identity() == scoop_identity::ConeIdentity::CORE {
        Vec::new()
    } else {
        vec![scoop_identity::ConeIdentity::CORE]
    };
    verify_production_code_projection_v1(
        cone,
        &dependency_identities,
        &legacy_image_dependencies,
        source_count,
        strong_production,
        link_objects,
    )
}

fn verify_production_code_projection_v1(
    cone: &ConeRecord,
    dependency_identities: &[scoop_identity::ConeIdentity],
    expected_image_dependencies: &[scoop_identity::ConeIdentity],
    source_count: usize,
    strong_production: StrongProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let projection = verify_production_code_projection_common(
        cone,
        dependency_identities,
        expected_image_dependencies,
        source_count,
        &strong_production,
        &link_objects,
    )?;
    Ok(VerifiedSingleConeProductionCodeProjectionV1 {
        strong_production,
        link_objects,
        projection,
    })
}

pub(super) fn verify_production_code_projection_common<D, C, I>(
    cone: &ConeRecord,
    dependency_identities: &[scoop_identity::ConeIdentity],
    expected_image_dependencies: &[scoop_identity::ConeIdentity],
    source_count: usize,
    strong_production: &StrongProductionSection<D, C, I>,
    link_objects: &VerifiedCodeLinkObjectMemberSetV1,
) -> Result<SingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let final_objects = link_objects.final_objects();
    let runtime_image = final_objects.runtime_images().fingerprint();
    let registrations = runtime_image.registrations();

    if link_objects.producer() != cone.identity()
        || strong_production.image_plan().cone().identity() != cone.identity()
        || strong_production.image_plan().cone().coordinate() != cone.coordinate()
    {
        return Err(ProductionCodeProjectionError::ConeMismatch);
    }
    if strong_production.digest_finalization_plan()
        != final_objects.entry().patch_sites().digest_plan()
    {
        return Err(ProductionCodeProjectionError::DigestPlanMismatch);
    }
    if strong_production.image_plan() != runtime_image.image().plan() {
        return Err(ProductionCodeProjectionError::ImagePlanMismatch);
    }
    if strong_production.entry_plan() != final_objects.entry().plan() {
        return Err(ProductionCodeProjectionError::EntryPlanMismatch);
    }
    if strong_production.generated_bridge_plan()
        != final_objects
            .entry()
            .patch_sites()
            .builtins()
            .c_bridge_production()
            .bridge_plan()
    {
        return Err(ProductionCodeProjectionError::GeneratedBridgePlanMismatch);
    }
    if !registration_identities_match(
        strong_production.registration_production().identities(),
        registrations,
    ) {
        return Err(ProductionCodeProjectionError::RegistrationIdentityMismatch);
    }

    if expected_image_dependencies != strong_production.image_plan().dependencies() {
        return Err(ProductionCodeProjectionError::DependencyMismatch);
    }

    let distribution = distribution(cone, dependency_identities, source_count)?;
    let output = output(cone, final_objects)?;
    let strong_registration_set =
        CanonicalStrongRegistrationFingerprintSetV1::from_patch_set(registrations)
            .map_err(ProductionCodeProjectionError::StrongRegistrations)?;
    let projection = SingleConeProductionCodeProjectionV1 {
        distribution,
        output,
        image_owner_member: runtime_image.image().member(),
        runtime_registration_projection: strong_production
            .registration_production()
            .identities()
            .clone(),
        strong_registration_set,
        runtime_image_fingerprint: runtime_image.fingerprint(),
    };
    Ok(projection)
}

fn distribution(
    cone: &ConeRecord,
    dependencies: &[scoop_identity::ConeIdentity],
    source_count: usize,
) -> Result<ArtifactDistributionClassV1, ProductionCodeProjectionError> {
    match cone.source_form() {
        ConeSourceForm::Manifest => Ok(ArtifactDistributionClassV1::DistributableCone),
        ConeSourceForm::SingleFile
            if cone.kind() == ConeKind::Executable
                && source_count == 1
                && dependencies == [scoop_identity::ConeIdentity::CORE] =>
        {
            Ok(ArtifactDistributionClassV1::LocalExecutableRoot)
        }
        ConeSourceForm::SingleFile => Err(ProductionCodeProjectionError::InvalidSingleFileRoot {
            kind: cone.kind(),
            source_count,
            dependencies: dependencies.to_vec(),
        }),
    }
}

fn output(
    cone: &ConeRecord,
    final_objects: &crate::link_object::VerifiedEntryPatchSetV1,
) -> Result<SingleConeProductionOutputV1, ProductionCodeProjectionError> {
    match (
        cone.kind(),
        final_objects.entry().plan(),
        final_objects.entry().branch(),
    ) {
        (
            ConeKind::Library,
            EntryProductionPlanV1::Library,
            VerifiedEntryProductionBranchV1::Library,
        ) => Ok(SingleConeProductionOutputV1::Library),
        (
            ConeKind::Executable,
            EntryProductionPlanV1::Executable(plan),
            VerifiedEntryProductionBranchV1::Executable(entry),
        ) => {
            let gateway = final_objects
                .runtime_images()
                .fingerprint()
                .registrations()
                .callables()
                .fingerprints()
                .iter()
                .find(|fingerprint| fingerprint.body() == plan.gateway())
                .copied()
                .ok_or(ProductionCodeProjectionError::MissingGatewayFingerprint(
                    plan.gateway(),
                ))?;
            Ok(SingleConeProductionOutputV1::Executable(Box::new(
                ExecutableRootProjectionV1 {
                    main: plan.main(),
                    source_signature_fingerprint: plan.source_signature_fingerprint(),
                    gateway: plan.gateway(),
                    gateway_definition_fingerprint: gateway.body_definition(),
                    failure_root: plan.failure_root(),
                    entry_owner_member: entry.member(),
                },
            )))
        }
        _ => Err(ProductionCodeProjectionError::OutputMismatch),
    }
}

fn registration_identities_match(
    identities: &StrongRegistrationIdentitySurfaceV1,
    registrations: &VerifiedStrongRegistrationPatchSetV1,
) -> bool {
    let static_storages = registrations
        .static_storages()
        .shapes()
        .storage_definitions()
        .registration_objects()
        .registrations()
        .plan()
        .registrations();
    let immortal_objects = registrations
        .immortal_objects()
        .object_definitions()
        .registration_objects()
        .registrations()
        .plan()
        .registrations();
    let initializations = registrations
        .initializations()
        .definitions()
        .registration_objects()
        .registrations()
        .plan()
        .registrations();
    let types = registrations
        .types()
        .dependencies()
        .registration_objects()
        .registrations()
        .plan()
        .registrations();
    let safepoints = registrations
        .safepoints()
        .registrations()
        .plan()
        .registrations();
    let callables = registrations
        .callables()
        .body_objects()
        .registration_objects()
        .registrations()
        .plan()
        .registrations();

    table_matches(
        identities.static_storages(),
        static_storages.iter().map(|plan| {
            (
                plan.semantic().storage(),
                plan.registration_definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    ) && table_matches(
        identities.immortal_objects(),
        immortal_objects.iter().map(|plan| {
            (
                plan.object(),
                plan.registration_definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    ) && table_matches(
        identities.initialization_units(),
        initializations.iter().map(|plan| {
            (
                plan.semantic().unit(),
                plan.registration_definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    ) && table_matches(
        identities.type_registrations(),
        types.iter().map(|plan| {
            (
                plan.exact_type(),
                plan.definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    ) && table_matches(
        identities.safepoints(),
        safepoints.iter().map(|plan| {
            (
                plan.site(),
                plan.definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    ) && table_matches(
        identities.callables(),
        callables.iter().map(|plan| {
            (
                plan.body(),
                plan.definition_plan(),
                plan.registration_fingerprint_node(),
            )
        }),
    )
}

fn table_matches<I: scoop_identity::PersistentId>(
    identities: &[StrongRegistrationIdentityV1<I>],
    plans: impl IntoIterator<Item = (I, ObjectDefinitionPlanId, DigestNodeId)>,
) -> bool {
    identities
        .iter()
        .map(|identity| {
            (
                identity.semantic_id(),
                identity.definition_plan(),
                identity.fingerprint_node(),
            )
        })
        .eq(plans)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionCodeProjectionError {
    ConeMismatch,
    DependencyMismatch,
    DigestPlanMismatch,
    ImagePlanMismatch,
    EntryPlanMismatch,
    GeneratedBridgePlanMismatch,
    RegistrationIdentityMismatch,
    InvalidSingleFileRoot {
        kind: ConeKind,
        source_count: usize,
        dependencies: Vec<scoop_identity::ConeIdentity>,
    },
    OutputMismatch,
    MissingGatewayFingerprint(PersistentCallableBodyId),
    StrongRegistrations(StrongRegistrationFingerprintProjectionError),
}

impl fmt::Display for ProductionCodeProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid production code projection: {self:?}")
    }
}

impl std::error::Error for ProductionCodeProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StrongRegistrations(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
