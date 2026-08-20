//! Canonical semantic inputs for the single-Cone code fingerprint.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeExternalRequirementSurfaceV1,
    CanonicalNativeLibraryRequirementV1, ConeProductionSectionV1,
};
use scoop_wire::{Digest256, Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    CrossConeLinkClosureBuildError, CrossConeLinkClosureSectionV1,
    DefinedLinkSymbolOwnerBuildError, FinalizedUndefinedSymbolRequirementPartitionsV1,
    VerifiedEntryPatchSetV1,
};
use crate::{
    CodeFingerprint, LinkMemberFingerprint, MemberStableKey, SingleConeProductionCodeProjectionV1,
    SlibMemberId, SlibMemberRecord, SlibMemberRole, VerifiedSingleConeProductionCodeProjectionV1,
};

const CODE_FINGERPRINT_DOMAIN: &str = "scoop-code-v1";

mod wire;
pub(in crate::link_object) use wire::DecodedCodeLinkObjectMemberSetV1;
pub use wire::{
    DecodedCanonicalNativeExternalContractCodeSetV1, NativeExternalContractCodeSetValidationError,
};

mod contributions;
pub use contributions::*;

mod directory;
pub use directory::CodeLinkObjectMemberValidationError;
#[cfg(test)]
use directory::ExpectedFinalLinkObjectMemberV1;
use directory::{expected_final_members, verify_directory_records};

/// One final LinkObject directory identity and its content-bound fingerprint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedCodeLinkObjectMemberV1 {
    member: SlibMemberId,
    fingerprint: LinkMemberFingerprint,
}

impl VerifiedCodeLinkObjectMemberV1 {
    pub const fn member(self) -> SlibMemberId {
        self.member
    }

    pub const fn fingerprint(self) -> LinkMemberFingerprint {
        self.fingerprint
    }
}

/// Finalized built-in object bytes bound one-to-one to their directory records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeLinkObjectMemberSetV1 {
    members: Vec<VerifiedCodeLinkObjectMemberV1>,
}

impl CodeLinkObjectMemberSetV1 {
    pub fn members(&self) -> &[VerifiedCodeLinkObjectMemberV1] {
        &self.members
    }
}

#[cfg(test)]
pub(in crate::link_object) fn empty_code_link_object_member_set_for_test()
-> CodeLinkObjectMemberSetV1 {
    CodeLinkObjectMemberSetV1 {
        members: Vec::new(),
    }
}

impl WireEncode for CodeLinkObjectMemberSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.members.len() as u64)?;
        for member in &self.members {
            member.fingerprint.encode(encoder)?;
        }
        Ok(())
    }
}

/// Finalized built-in object bytes bound one-to-one to their directory records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCodeLinkObjectMemberSetV1<
    D = scoop_lir::StrongTypeDescriptorRefV1,
    C = scoop_lir::StrongTypeDispatchCallableRefV1,
    I = scoop_identity::PersistentInitializationUnitId,
> {
    final_objects: VerifiedEntryPatchSetV1<D, C, I>,
    projection: CodeLinkObjectMemberSetV1,
}

pub type VerifiedCodeLinkObjectMemberSetV2 = VerifiedCodeLinkObjectMemberSetV1<
    scoop_lir::StrongTypeDescriptorRefV2,
    scoop_lir::StrongTypeDispatchCallableRefV2,
    scoop_lir::StrongInitializationDependencyRefV2,
>;

impl<D: scoop_lir::StrongDescriptorReference, C: Clone, I>
    VerifiedCodeLinkObjectMemberSetV1<D, C, I>
{
    pub const fn producer(&self) -> ConeIdentity {
        self.final_objects.entry().patch_sites().producer()
    }

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV1<D, C, I> {
        &self.final_objects
    }

    pub fn members(&self) -> &[VerifiedCodeLinkObjectMemberV1] {
        self.projection.members()
    }

    pub const fn projection(&self) -> &CodeLinkObjectMemberSetV1 {
        &self.projection
    }
}

/// Bind every built-in LinkObject directory record to the final verified bytes.
pub fn verify_code_link_object_members_v1(
    final_objects: VerifiedEntryPatchSetV1,
    directory: &[SlibMemberRecord],
) -> Result<VerifiedCodeLinkObjectMemberSetV1, CodeLinkObjectMemberValidationError> {
    verify_code_link_object_members(final_objects, directory)
}

pub fn verify_code_link_object_members_v2(
    final_objects: crate::VerifiedEntryPatchSetV2,
    directory: &[SlibMemberRecord],
) -> Result<VerifiedCodeLinkObjectMemberSetV2, CodeLinkObjectMemberValidationError> {
    verify_code_link_object_members(final_objects, directory)
}

fn verify_code_link_object_members<D, C, I>(
    final_objects: VerifiedEntryPatchSetV1<D, C, I>,
    directory: &[SlibMemberRecord],
) -> Result<VerifiedCodeLinkObjectMemberSetV1<D, C, I>, CodeLinkObjectMemberValidationError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let expected = expected_final_members(&final_objects)?;
    let projection = verify_directory_records(&expected, directory)?;
    Ok(VerifiedCodeLinkObjectMemberSetV1 {
        final_objects,
        projection,
    })
}

/// A code digest together with every proof and canonical projection from
/// which its nine-field input was encoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCodeFingerprintV1 {
    production: VerifiedSingleConeProductionCodeProjectionV1,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    native_contracts: CanonicalNativeExternalContractCodeSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    fingerprint: CodeFingerprint,
}

impl VerifiedCodeFingerprintV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.production.link_objects().producer()
    }

    pub const fn production(&self) -> &VerifiedSingleConeProductionCodeProjectionV1 {
        &self.production
    }

    pub const fn link_extension_contributions(
        &self,
    ) -> &CanonicalKnownLinkExtensionCodeContributionSetV1 {
        &self.link_extension_contributions
    }

    pub const fn native_requirements(&self) -> &CanonicalNativeExternalRequirementSurfaceV1 {
        &self.native_requirements
    }

    pub const fn native_contracts(&self) -> &CanonicalNativeExternalContractCodeSetV1 {
        &self.native_contracts
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn fingerprint(&self) -> CodeFingerprint {
        self.fingerprint
    }

    pub const fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        self.production
            .link_objects()
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .c_bridge_production()
            .production()
    }
}

pub fn compute_code_fingerprint_v1(
    production: VerifiedSingleConeProductionCodeProjectionV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<VerifiedCodeFingerprintV1, CodeFingerprintError> {
    let builtins = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins();
    if !undefined_symbols.matches_strong_closure(builtins.strong_relocations()) {
        return Err(CodeFingerprintError::UndefinedSymbolSetMismatch);
    }
    compute_code_fingerprint_with_contributions_v1(
        production,
        CanonicalKnownLinkExtensionCodeContributionSetV1::empty(),
        native_requirements,
        defined_symbols,
        undefined_symbols,
    )
}

/// Code proof and Link-only closure for the M23-5 cross-Cone profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCrossConeCodeFingerprintV1 {
    code: VerifiedCodeFingerprintV1,
    link_closure: CrossConeLinkClosureSectionV1,
}

impl VerifiedCrossConeCodeFingerprintV1 {
    pub const fn code(&self) -> &VerifiedCodeFingerprintV1 {
        &self.code
    }

    pub const fn link_closure(&self) -> &CrossConeLinkClosureSectionV1 {
        &self.link_closure
    }

    pub fn into_parts(self) -> (VerifiedCodeFingerprintV1, CrossConeLinkClosureSectionV1) {
        (self.code, self.link_closure)
    }
}

pub fn compute_cross_cone_code_fingerprint_v1(
    production: VerifiedSingleConeProductionCodeProjectionV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedUndefinedSymbolRequirementPartitionsV1,
) -> Result<VerifiedCrossConeCodeFingerprintV1, CodeFingerprintError> {
    let builtins = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins();
    if !undefined_partitions.matches_strong_closure(builtins.strong_relocations()) {
        return Err(CodeFingerprintError::UndefinedSymbolPartitionMismatch);
    }
    let link_closure = CrossConeLinkClosureSectionV1::from_verified_requirements(
        undefined_partitions.cross_cone(),
        production.link_objects(),
    )
    .map_err(CodeFingerprintError::CrossConeLinkClosure)?;
    let link_extension_contributions =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_semantic_imports(
            link_closure.semantic_imports(),
        )
        .map_err(CodeFingerprintError::LinkContributionEncoding)?;
    let (undefined_symbols, _) = undefined_partitions.into_parts();
    let code = compute_code_fingerprint_with_contributions_v1(
        production,
        link_extension_contributions,
        native_requirements,
        defined_symbols,
        undefined_symbols,
    )?;
    Ok(VerifiedCrossConeCodeFingerprintV1 { code, link_closure })
}

fn compute_code_fingerprint_with_contributions_v1(
    production: VerifiedSingleConeProductionCodeProjectionV1,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<VerifiedCodeFingerprintV1, CodeFingerprintError> {
    let producer = production.link_objects().producer();
    let target = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins()
        .strong_relocations()
        .target();
    if native_requirements.producer() != producer {
        return Err(CodeFingerprintError::NativeProducerMismatch {
            expected: producer,
            actual: native_requirements.producer(),
        });
    }
    if native_requirements.target() != target
        || undefined_symbols.producer() != producer
        || undefined_symbols.selection().target() != target
    {
        return Err(CodeFingerprintError::TargetMismatch);
    }
    let builtins = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins();
    let expected_defined = CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
        builtins.strong_relocations(),
    )
    .map_err(CodeFingerprintError::DefinedSymbols)?;
    if defined_symbols != expected_defined {
        return Err(CodeFingerprintError::DefinedSymbolSetMismatch);
    }
    let native_contracts =
        CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(&native_requirements)
            .map_err(CodeFingerprintError::NativeContracts)?;
    let fingerprint = domain_separated_cbor_hash(
        CODE_FINGERPRINT_DOMAIN,
        &CodeFingerprintInputV1 {
            production: &production,
            link_extension_contributions: &link_extension_contributions,
            native_requirements: &native_requirements,
            native_contracts: &native_contracts,
            defined_symbols: &defined_symbols,
            undefined_symbols: &undefined_symbols,
        },
    )
    .map(|digest| CodeFingerprint::from_array(*digest.as_array()))
    .map_err(CodeFingerprintError::Hash)?;
    Ok(VerifiedCodeFingerprintV1 {
        production,
        link_extension_contributions,
        native_requirements,
        native_contracts,
        defined_symbols,
        undefined_symbols,
        fingerprint,
    })
}

struct CodeFingerprintInputV1<'proof> {
    production: &'proof VerifiedSingleConeProductionCodeProjectionV1,
    link_extension_contributions: &'proof CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: &'proof CanonicalNativeExternalRequirementSurfaceV1,
    native_contracts: &'proof CanonicalNativeExternalContractCodeSetV1,
    defined_symbols: &'proof CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: &'proof CanonicalUndefinedSymbolRequirementSetV1,
}

impl WireEncode for CodeFingerprintInputV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.production
            .link_objects()
            .projection()
            .encode(encoder)?;
        encoder.field(2)?;
        self.link_extension_contributions.encode(encoder)?;
        encoder.field(3)?;
        self.c_bridge_production().encode(encoder)?;
        encoder.field(4)?;
        encode_native_library_requirements(
            encoder,
            self.native_requirements.library_requirements(),
        )?;
        encoder.field(5)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(6)?;
        self.undefined_symbols.encode(encoder)?;
        encoder.field(7)?;
        self.native_contracts.encode(encoder)?;
        encoder.field(8)?;
        self.strong_production().encode(encoder)?;
        encoder.field(9)?;
        self.manifest_projection().encode(encoder)?;
        encoder.field(10)?;
        encoder.unsigned(u64::from(self.native_requirements.cxx()))
    }
}

impl CodeFingerprintInputV1<'_> {
    fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        self.production
            .link_objects()
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .c_bridge_production()
            .production()
    }

    fn strong_production(&self) -> &ConeProductionSectionV1 {
        self.production.strong_production()
    }

    fn manifest_projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        self.production.projection()
    }
}

fn encode_native_library_requirements(
    encoder: &mut Encoder,
    requirements: &[CanonicalNativeLibraryRequirementV1],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(requirements.len() as u64)?;
    for requirement in requirements {
        requirement.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeFingerprintError {
    NativeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    TargetMismatch,
    DefinedSymbols(DefinedLinkSymbolOwnerBuildError),
    DefinedSymbolSetMismatch,
    UndefinedSymbolSetMismatch,
    UndefinedSymbolPartitionMismatch,
    CrossConeLinkClosure(CrossConeLinkClosureBuildError),
    LinkContributionEncoding(scoop_wire::cbor::EncodeError),
    NativeContracts(NativeExternalContractCodeSetBuildError),
    Hash(HashError),
}

impl fmt::Display for CodeFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "failed to compute code fingerprint: {self:?}")
    }
}

impl std::error::Error for CodeFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinedSymbols(source) => Some(source),
            Self::CrossConeLinkClosure(source) => Some(source),
            Self::LinkContributionEncoding(source) => Some(source),
            Self::NativeContracts(source) => Some(source),
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
