//! Final old/new undefined-use partitions for the cross-Cone profile.

use super::{
    CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementFinalizationError,
    finalize_partitioned_undefined_symbol_requirements_inner,
};
use crate::link_object::{
    SealedBuiltinObjectExternalRequirementClosureV1, VerifiedCrossConeStrongRequirementClosureV1,
    VerifiedCurrentConeUndefinedRequirementClosureV1,
};

/// Mutually exclusive legacy and ordinary-dependency relocation closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedUndefinedSymbolRequirementPartitionsV1 {
    legacy: CanonicalUndefinedSymbolRequirementSetV1,
    cross_cone: VerifiedCrossConeStrongRequirementClosureV1,
}

impl FinalizedUndefinedSymbolRequirementPartitionsV1 {
    pub const fn legacy(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.legacy
    }

    pub const fn cross_cone(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        &self.cross_cone
    }

    pub fn into_parts(
        self,
    ) -> (
        CanonicalUndefinedSymbolRequirementSetV1,
        VerifiedCrossConeStrongRequirementClosureV1,
    ) {
        (self.legacy, self.cross_cone)
    }
}

pub fn finalize_partitioned_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
) -> Result<
    FinalizedUndefinedSymbolRequirementPartitionsV1,
    UndefinedSymbolRequirementFinalizationError,
> {
    let cross_cone = external.cross_cone_closure().clone();
    let legacy = finalize_partitioned_undefined_symbol_requirements_inner(current_cone, external)?;
    Ok(FinalizedUndefinedSymbolRequirementPartitionsV1 { legacy, cross_cone })
}
