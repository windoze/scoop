//! Code fingerprint closure for the M23-6 layout profile.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{CBridgeProductionSetV1, CanonicalNativeExternalRequirementSurfaceV1};
use scoop_wire::HashError;

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalKnownLinkExtensionCodeContributionSetV1,
    CanonicalNativeExternalContractCodeSetV1, CanonicalUndefinedSymbolRequirementSetV1,
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1, NativeExternalContractCodeSetBuildError,
    VerifiedExternalShapeRequirementClosureV1,
};
use crate::{CodeFingerprint, VerifiedSingleConeProductionCodeProjectionV2};

mod error;
mod input;
pub use error::LayoutCodeFingerprintError;
pub(crate) use input::LayoutCodeFingerprintInputV1;

/// A Code digest whose field 8 is the exact Strong V2 production section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeFingerprintDataV2 {
    production: VerifiedSingleConeProductionCodeProjectionV2,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    native_contracts: CanonicalNativeExternalContractCodeSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    fingerprint: CodeFingerprint,
}

impl CodeFingerprintDataV2 {
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

/// The generated Code result and runtime aliases used by the artifact writer.
#[derive(Clone, Debug)]
pub struct CrossConeLayoutCodeDataV1 {
    code: CodeFingerprintDataV2,
    link_support: crate::LirLinkSupportSectionV1,
}

impl CrossConeLayoutCodeDataV1 {
    pub const fn code(&self) -> &CodeFingerprintDataV2 {
        &self.code
    }
    pub fn into_parts(self) -> (CodeFingerprintDataV2, crate::LirLinkSupportSectionV1) {
        (self.code, self.link_support)
    }
}

/// Encodes the generated objects, requirements, and semantic imports into Code.
pub fn compute_cross_cone_layout_code_fingerprint_v1(
    production: VerifiedSingleConeProductionCodeProjectionV2,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    external_shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
    link_support: crate::LirLinkSupportSectionV1,
) -> Result<CrossConeLayoutCodeDataV1, LayoutCodeFingerprintError> {
    let link_extension_contributions =
        CanonicalKnownLinkExtensionCodeContributionSetV1::from_cross_cone_layout_semantic_imports(
            undefined_partitions.cross_cone().semantic_imports(),
            external_shape.semantic_imports(),
        )
        .map_err(LayoutCodeFingerprintError::LinkContributionEncoding)?;
    let link_extension_contributions = link_extension_contributions
        .with_link_support(&link_support)
        .map_err(LayoutCodeFingerprintError::LinkContributionEncoding)?;
    let (undefined_symbols, _, _) = undefined_partitions.into_parts();
    let code = compute(
        production,
        link_extension_contributions,
        native_requirements,
        defined_symbols,
        undefined_symbols,
    )?;
    Ok(CrossConeLayoutCodeDataV1 { code, link_support })
}

fn compute(
    production: VerifiedSingleConeProductionCodeProjectionV2,
    link_extension_contributions: CanonicalKnownLinkExtensionCodeContributionSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<CodeFingerprintDataV2, LayoutCodeFingerprintError> {
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
    Ok(CodeFingerprintDataV2 {
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
