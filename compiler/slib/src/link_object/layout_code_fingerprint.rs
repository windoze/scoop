//! Code fingerprint closure for the M23-6 layout profile.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeExternalRequirementSurfaceV1,
    CanonicalNativeLibraryRequirementV1, LirTargetProfile, StrongProductionSectionV2,
    ValidatedLirTargetSelection,
};
use scoop_wire::{BudgetMeter, Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalKnownLinkExtensionCodeContributionSetV1,
    CanonicalNativeExternalContractCodeSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    CrossConeLinkClosureBuildError, CrossConeLinkClosureSectionV1,
    DefinedLinkSymbolOwnerBuildError, FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    LayoutLinkClosureError, NativeExternalContractCodeSetBuildError,
    VerifiedExternalShapeRequirementClosureV1,
};
use crate::{
    CodeFingerprint, CrossConeLayoutLinkClosureSectionV1,
    VerifiedSingleConeProductionCodeProjectionV2,
};

const CODE_FINGERPRINT_DOMAIN: &str = "scoop-code-v1";

/// A Code digest whose field 8 is the exact Strong V2 production section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCodeFingerprintV2 {
    production: VerifiedSingleConeProductionCodeProjectionV2,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    native_contracts: CanonicalNativeExternalContractCodeSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    fingerprint: CodeFingerprint,
}

impl VerifiedCodeFingerprintV2 {
    pub const fn producer(&self) -> ConeIdentity {
        self.production.link_objects().producer()
    }

    pub const fn production(&self) -> &VerifiedSingleConeProductionCodeProjectionV2 {
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

/// Complete Code proof plus the two Link-only sections whose semantic import
/// projections are committed by that digest.
#[derive(Clone, Debug)]
pub struct VerifiedCrossConeLayoutCodeFingerprintV1<'a> {
    code: VerifiedCodeFingerprintV2,
    callable_link_closure: CrossConeLinkClosureSectionV1,
    layout_link_closure: CrossConeLayoutLinkClosureSectionV1<'a>,
}

impl<'a> VerifiedCrossConeLayoutCodeFingerprintV1<'a> {
    pub const fn code(&self) -> &VerifiedCodeFingerprintV2 {
        &self.code
    }

    pub const fn callable_link_closure(&self) -> &CrossConeLinkClosureSectionV1 {
        &self.callable_link_closure
    }

    pub const fn layout_link_closure(&self) -> &CrossConeLayoutLinkClosureSectionV1<'a> {
        &self.layout_link_closure
    }

    pub fn into_parts(
        self,
    ) -> (
        VerifiedCodeFingerprintV2,
        CrossConeLinkClosureSectionV1,
        CrossConeLayoutLinkClosureSectionV1<'a>,
    ) {
        (
            self.code,
            self.callable_link_closure,
            self.layout_link_closure,
        )
    }
}

/// Computes the M23-6 Code digest only after the three undefined-symbol
/// partitions and both semantic import projections have been bound to the
/// exact same finalized object proof.
pub fn compute_cross_cone_layout_code_fingerprint_v1<'a>(
    production: VerifiedSingleConeProductionCodeProjectionV2,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    external_shape: &'a VerifiedExternalShapeRequirementClosureV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<VerifiedCrossConeLayoutCodeFingerprintV1<'a>, LayoutCodeFingerprintError> {
    let builtins = production
        .link_objects()
        .final_objects()
        .entry()
        .patch_sites()
        .builtins();
    if !undefined_partitions.matches_strong_closure(builtins.strong_relocations()) {
        return Err(LayoutCodeFingerprintError::UndefinedSymbolPartitionMismatch);
    }
    if !undefined_partitions.matches_external_shape_closure(external_shape) {
        return Err(LayoutCodeFingerprintError::ExternalShapeClosureMismatch);
    }
    let callable_link_closure = CrossConeLinkClosureSectionV1::from_verified_requirements(
        undefined_partitions.cross_cone(),
        production.link_objects(),
    )
    .map_err(LayoutCodeFingerprintError::CrossConeLinkClosure)?;
    let layout_link_closure = CrossConeLayoutLinkClosureSectionV1::from_verified_requirements(
        external_shape,
        production.link_objects(),
        meter,
    )
    .map_err(LayoutCodeFingerprintError::LayoutLinkClosure)?;
    let link_extension_contributions =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_layout_semantic_imports(
            callable_link_closure.semantic_imports(),
            layout_link_closure.semantic_imports(),
        )
        .map_err(LayoutCodeFingerprintError::LinkContributionEncoding)?;
    let (undefined_symbols, _, _) = undefined_partitions.into_parts();
    let code = compute(
        production,
        link_extension_contributions,
        native_requirements,
        defined_symbols,
        undefined_symbols,
    )?;
    Ok(VerifiedCrossConeLayoutCodeFingerprintV1 {
        code,
        callable_link_closure,
        layout_link_closure,
    })
}

fn compute(
    production: VerifiedSingleConeProductionCodeProjectionV2,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<VerifiedCodeFingerprintV2, LayoutCodeFingerprintError> {
    let producer = production.link_objects().producer();
    if native_requirements.producer() != producer {
        return Err(LayoutCodeFingerprintError::NativeProducerMismatch {
            expected: producer,
            actual: native_requirements.producer(),
        });
    }
    if native_requirements.target() != LirTargetProfile::DARWIN_AARCH64
        || undefined_symbols.producer() != producer
        || undefined_symbols.selection() != ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
    {
        return Err(LayoutCodeFingerprintError::TargetMismatch);
    }
    let expected_defined = CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
        production
            .link_objects()
            .final_objects()
            .entry()
            .patch_sites()
            .builtins()
            .strong_relocations(),
    )
    .map_err(LayoutCodeFingerprintError::DefinedSymbols)?;
    if defined_symbols != expected_defined {
        return Err(LayoutCodeFingerprintError::DefinedSymbolSetMismatch);
    }
    let native_contracts =
        CanonicalNativeExternalContractCodeSetV1::from_requirement_surface(&native_requirements)
            .map_err(LayoutCodeFingerprintError::NativeContracts)?;
    let fingerprint = domain_separated_cbor_hash(
        CODE_FINGERPRINT_DOMAIN,
        &LayoutCodeFingerprintInputV1 {
            production: &production,
            link_extension_contributions: &link_extension_contributions,
            native_requirements: &native_requirements,
            native_contracts: &native_contracts,
            defined_symbols: &defined_symbols,
            undefined_symbols: &undefined_symbols,
        },
    )
    .map(|digest| CodeFingerprint::from_array(*digest.as_array()))
    .map_err(LayoutCodeFingerprintError::Hash)?;
    Ok(VerifiedCodeFingerprintV2 {
        production,
        link_extension_contributions,
        native_requirements,
        native_contracts,
        defined_symbols,
        undefined_symbols,
        fingerprint,
    })
}

struct LayoutCodeFingerprintInputV1<'proof> {
    production: &'proof VerifiedSingleConeProductionCodeProjectionV2,
    link_extension_contributions: &'proof CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: &'proof CanonicalNativeExternalRequirementSurfaceV1,
    native_contracts: &'proof CanonicalNativeExternalContractCodeSetV1,
    defined_symbols: &'proof CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: &'proof CanonicalUndefinedSymbolRequirementSetV1,
}

impl WireEncode for LayoutCodeFingerprintInputV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
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
        self.production.projection().encode(encoder)
    }
}

impl LayoutCodeFingerprintInputV1<'_> {
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

    fn strong_production(&self) -> &StrongProductionSectionV2 {
        self.production.strong_production()
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

#[derive(Debug)]
pub enum LayoutCodeFingerprintError {
    NativeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    TargetMismatch,
    DefinedSymbols(DefinedLinkSymbolOwnerBuildError),
    DefinedSymbolSetMismatch,
    UndefinedSymbolPartitionMismatch,
    ExternalShapeClosureMismatch,
    CrossConeLinkClosure(CrossConeLinkClosureBuildError),
    LayoutLinkClosure(LayoutLinkClosureError),
    LinkContributionEncoding(scoop_wire::cbor::EncodeError),
    NativeContracts(NativeExternalContractCodeSetBuildError),
    Hash(HashError),
}

impl fmt::Display for LayoutCodeFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute layout code fingerprint: {self:?}"
        )
    }
}

impl std::error::Error for LayoutCodeFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DefinedSymbols(source) => Some(source),
            Self::CrossConeLinkClosure(source) => Some(source),
            Self::LayoutLinkClosure(source) => Some(source),
            Self::LinkContributionEncoding(source) => Some(source),
            Self::NativeContracts(source) => Some(source),
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::{EmptyLayoutCodeFixture, with_empty_layout_code_fixture};
