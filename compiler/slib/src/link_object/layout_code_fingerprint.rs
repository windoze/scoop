//! Code fingerprint closure for the M23-6 layout profile.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeExternalRequirementSurfaceV1, LirTargetProfile,
    ValidatedLirTargetSelection,
};
use scoop_wire::{BudgetMeter, HashError};

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

mod error;
mod input;
pub use error::LayoutCodeFingerprintError;
pub(crate) use input::LayoutCodeFingerprintInputV1;

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
    let fingerprint = LayoutCodeFingerprintInputV1 {
        objects: production.link_objects(),
        production: production.projection(),
        strong: production.strong_production(),
        link_extension_contributions: &link_extension_contributions,
        native_requirements: &native_requirements,
        native_contracts: &native_contracts,
        defined_symbols: &defined_symbols,
        undefined_symbols: &undefined_symbols,
    }
    .fingerprint()?;
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

#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::{EmptyLayoutCodeFixture, with_empty_layout_code_fixture};
