//! Undefined-use partitions for the M23-6 layout profile.

use scoop_identity::ConeIdentity;
use scoop_lir::ExternalStrongShapeSubjectV1;

use super::CanonicalObjectDefinitionRequirementV1;
use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    UndefinedSymbolRequirementFinalizationError, VerifiedCrossConeStrongRequirementClosureV1,
    VerifiedCurrentConeUndefinedRequirementClosureV1, VerifiedExternalShapeRequirementClosureV1,
    VerifiedRuntimeAndEhRequirementClosureV1, finalize_undefined_symbol_requirements_v1,
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
    external: VerifiedRuntimeAndEhRequirementClosureV1,
    external_shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
) -> Result<
    FinalizedLayoutUndefinedSymbolRequirementPartitionsV1,
    UndefinedSymbolRequirementFinalizationError,
> {
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
    let cross_cone = external.source_closure().cross_cone_closure().clone();
    let legacy = finalize_undefined_symbol_requirements_v1(current_cone, external);
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
