//! Owned three-way undefined-use proof for the M23-6 layout profile.

use scoop_identity::ConeIdentity;
use scoop_lir::ExternalStrongShapeSubjectV1;

use super::CanonicalObjectDefinitionRequirementV1;
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    SealedBuiltinObjectExternalRequirementClosureV1, StrongRelocationResolutionV1,
    UndefinedSymbolRequirementFinalizationError, VerifiedCrossConeStrongRequirementClosureV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
    VerifiedExternalShapeRequirementClosureV1,
    finalize_partitioned_undefined_symbol_requirements_with_additional_uses_inner, use_key,
};

/// One general-shape use projected from the verified Link closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedExternalShapeUndefinedRequirementV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
}

impl FinalizedExternalShapeUndefinedRequirementV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn subject(&self) -> ExternalStrongShapeSubjectV1 {
        self.subject
    }
}

/// Mutually exclusive legacy, M23-5 callable, and M23-6 general-shape
/// relocation partitions. The shape records can only be projected from a
/// complete `VerifiedExternalShapeRequirementClosureV1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedLayoutUndefinedSymbolRequirementPartitionsV1 {
    legacy: CanonicalUndefinedSymbolRequirementSetV1,
    cross_cone: VerifiedCrossConeStrongRequirementClosureV1,
    external_shape: Vec<FinalizedExternalShapeUndefinedRequirementV1>,
}

impl FinalizedLayoutUndefinedSymbolRequirementPartitionsV1 {
    pub const fn legacy(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.legacy
    }

    pub const fn cross_cone(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        &self.cross_cone
    }

    pub fn external_shape(&self) -> &[FinalizedExternalShapeUndefinedRequirementV1] {
        &self.external_shape
    }

    pub fn into_parts(
        self,
    ) -> (
        CanonicalUndefinedSymbolRequirementSetV1,
        VerifiedCrossConeStrongRequirementClosureV1,
        Vec<FinalizedExternalShapeUndefinedRequirementV1>,
    ) {
        (self.legacy, self.cross_cone, self.external_shape)
    }

    pub(in crate::link_object) fn matches_strong_closure(
        &self,
        closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> bool {
        if self.legacy.producer() != closure.producer()
            || self.cross_cone.producer() != closure.producer()
            || self.legacy.selection().target() != closure.target()
            || self.cross_cone.target() != closure.target()
        {
            return false;
        }

        let mut expected = closure
            .bindings()
            .iter()
            .filter(|binding| {
                !matches!(
                    binding.resolution(),
                    StrongRelocationResolutionV1::ObjectLocalStrong { .. }
                )
            })
            .map(CanonicalUndefinedRelocationUseV1::from)
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(use_key);

        let mut actual = self
            .legacy
            .requirements()
            .iter()
            .map(|requirement| requirement.use_site().clone())
            .chain(
                self.cross_cone
                    .requirements()
                    .iter()
                    .map(|requirement| requirement.use_site().clone()),
            )
            .chain(
                self.external_shape
                    .iter()
                    .map(|requirement| requirement.use_site().clone()),
            )
            .collect::<Vec<_>>();
        actual.sort_unstable_by_key(use_key);
        actual == expected
    }

    pub(in crate::link_object) fn matches_external_shape_closure(
        &self,
        closure: &VerifiedExternalShapeRequirementClosureV1<'_>,
    ) -> bool {
        if &self.cross_cone != closure.legacy_closure()
            || self.external_shape.len() != closure.requirements().len()
        {
            return false;
        }
        self.external_shape
            .iter()
            .zip(closure.requirements())
            .all(|(finalized, classified)| {
                let Some(import) = closure
                    .semantic_imports()
                    .records()
                    .get(classified.import_index() as usize)
                else {
                    return false;
                };
                finalized.use_site() == classified.use_site()
                    && finalized.provider() == import.provider()
                    && finalized.subject() == import.subject()
            })
    }

    pub(in crate::link_object) fn shape_requirement_for(
        &self,
        use_site: &CanonicalUndefinedRelocationUseV1,
    ) -> Option<CanonicalObjectDefinitionRequirementV1> {
        let requirement = self
            .external_shape
            .iter()
            .find(|requirement| requirement.use_site() == use_site)?;
        Some(
            CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong {
                provider: requirement.provider(),
                subject: requirement.subject(),
            },
        )
    }
}

/// Finalizes all three M23-6 partitions against the same relocation closure.
pub fn finalize_layout_partitioned_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
    external_shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
) -> Result<
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    UndefinedSymbolRequirementFinalizationError,
> {
    if external.cross_cone_closure() != external_shape.legacy_closure() {
        return Err(UndefinedSymbolRequirementFinalizationError::ExternalShapeClosureMismatch);
    }

    let mut shape_requirements = Vec::with_capacity(external_shape.requirements().len());
    for requirement in external_shape.requirements() {
        let import_index = requirement.import_index();
        let import = external_shape
            .semantic_imports()
            .records()
            .get(import_index as usize)
            .ok_or(
                UndefinedSymbolRequirementFinalizationError::ExternalShapeImportIndexOutOfBounds {
                    import_index,
                    imports: external_shape.semantic_imports().records().len(),
                },
            )?;
        shape_requirements.push(FinalizedExternalShapeUndefinedRequirementV1 {
            use_site: requirement.use_site().clone(),
            provider: import.provider(),
            subject: import.subject(),
        });
    }
    let shape_uses = shape_requirements
        .iter()
        .map(|requirement| requirement.use_site().clone())
        .collect::<Vec<_>>();
    let cross_cone = external.cross_cone_closure().clone();
    let legacy = finalize_partitioned_undefined_symbol_requirements_with_additional_uses_inner(
        current_cone,
        external,
        &shape_uses,
    )?;
    Ok(FinalizedLayoutUndefinedSymbolRequirementPartitionsV1 {
        legacy,
        cross_cone,
        external_shape: shape_requirements,
    })
}

impl From<FinalizedLayoutUndefinedSymbolRequirementPartitionsV1>
    for super::VerifiedObjectDefinitionRequirementSetV1
{
    fn from(partitions: FinalizedLayoutUndefinedSymbolRequirementPartitionsV1) -> Self {
        Self {
            mode: super::ObjectDefinitionRequirementModeV1::Layout(Box::new(partitions)),
        }
    }
}
