use super::*;
use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WirePath};

/// Complete provider-local source tables plus the exact committed external-use
/// closure. Only `validate_semantics` can construct this publication evidence.
#[derive(Debug)]
pub struct CheckedCrossConeTypeSemanticsSectionV1<'a> {
    pub(super) exports: exports::CheckedTypeSectionExportsV1<'a>,
    selected: Vec<CheckedSelectedTypeUseV1<'a>>,
}
impl<'a> CheckedCrossConeTypeSemanticsSectionV1<'a> {
    pub const fn provider(&self) -> ConeIdentity {
        self.exports.provider
    }
    pub const fn section(&self) -> &'a CrossConeTypeSemanticsSectionV1 {
        self.exports.candidate
    }
    pub const fn public(&self) -> CheckedTypeSectionPublicSupportV1<'a> {
        self.exports.public
    }
    pub const fn source_roots(&self) -> &'a [SourceNominalId] {
        self.exports.source_roots
    }
    pub fn graph(&self) -> &CheckedNominalInheritanceGraphV1<'a> {
        &self.exports.graph
    }
    pub const fn facts(&self) -> CheckedExactTypeFactsV1<'a> {
        self.exports.facts
    }
    pub const fn representations(&self) -> CheckedNominalRepresentationSupportV1<'a> {
        self.exports.representations
    }
    pub const fn inheritance(&self) -> CheckedNominalInheritanceInterfacesV1<'a> {
        self.exports.inheritance
    }
    pub const fn declarations(&self) -> CheckedProtectedDeclarationSourcesV1<'a> {
        self.exports.protected
    }
    pub fn sources(&self) -> &CheckedProtectedSourceInterfacesV1<'a> {
        &self.exports.sources
    }
    pub fn defaults(&self) -> &CheckedProtectedDefaultTemplatesV1<'a> {
        &self.exports.defaults
    }
    pub fn selected(&self) -> &[CheckedSelectedTypeUseV1<'a>] {
        &self.selected
    }
    /// Includes borrowed terminal records, retaining provider ownership in
    /// their complete sections. No foreign record is copied into local wire.
    pub fn inheritance_record(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<&'a NominalInheritanceInterfaceV1> {
        self.exports.inheritance_records.get(&exact).copied()
    }
}

impl CrossConeTypeSemanticsSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_semantics<'a, F, S, D, A, E>(
        &'a self,
        public: CheckedTypeSectionPublicSupportV1<'a>,
        dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
        foundation: &'a F,
        declarations: &mut S,
        defaults: &mut D,
        committed: &A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CheckedCrossConeTypeSemanticsSectionV1<'a>, TypeSectionSemanticValidationError<E>>
    where
        F: TypeSectionFoundationSemanticAuthority<E>,
        S: TypeSectionDeclarationSemanticAuthority<E>,
        D: TypeSectionDefaultSemanticAuthority<E>,
        A: CommittedTypeUseSemanticAuthorityV1<E>,
    {
        let exports = exports::validate(
            self,
            public,
            dependencies,
            foundation,
            declarations,
            defaults,
            meter,
            path,
        )
        .map_err(|error| TypeSectionSemanticValidationError::Exports(Box::new(error)))?;
        let selected = selection::validate(
            &exports,
            dependencies,
            foundation,
            committed,
            meter,
            &path.clone().field(8),
        )
        .map_err(|error| TypeSectionSemanticValidationError::Selected(Box::new(error)))?;
        Ok(CheckedCrossConeTypeSemanticsSectionV1 { exports, selected })
    }
}

#[derive(Debug)]
pub enum TypeSectionSemanticValidationError<E> {
    Exports(Box<TypeSectionExportValidationError<E>>),
    Selected(Box<TypeSelectionValidationError<E>>),
}
impl<E: std::fmt::Display> std::fmt::Display for TypeSectionSemanticValidationError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exports(error) => error.fmt(f),
            Self::Selected(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSectionSemanticValidationError<E> {}
