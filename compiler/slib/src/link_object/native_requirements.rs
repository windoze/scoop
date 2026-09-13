//! Typed resolution of source-native external relocation requirements.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CanonicalNativeExternalRequirementSurfaceV1, CanonicalNativeExternalRequirementV1,
    LirTargetProfile,
};

use super::{StrongRelocationBindingV1, VerifiedCoreStrongRequirementClosureV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceExternalRequirementUseV1 {
    use_site: StrongRelocationBindingV1,
    requirement: CanonicalNativeExternalRequirementV1,
}

impl SourceExternalRequirementUseV1 {
    pub const fn use_site(&self) -> &StrongRelocationBindingV1 {
        &self.use_site
    }

    pub const fn requirement(&self) -> &CanonicalNativeExternalRequirementV1 {
        &self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSourceExternalRequirementClosureV1 {
    core_closure: VerifiedCoreStrongRequirementClosureV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    source_external_requirements: Vec<SourceExternalRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedSourceExternalRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.core_closure.producer()
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.core_closure.target()
    }

    pub const fn core_closure(&self) -> &VerifiedCoreStrongRequirementClosureV1 {
        &self.core_closure
    }

    pub const fn native_requirements(&self) -> &CanonicalNativeExternalRequirementSurfaceV1 {
        &self.native_requirements
    }

    pub fn source_external_requirements(&self) -> &[SourceExternalRequirementUseV1] {
        &self.source_external_requirements
    }

    pub fn remaining_external_candidates(&self) -> &[StrongRelocationBindingV1] {
        &self.remaining_external_candidates
    }
}

pub fn verify_source_external_requirements_v1(
    core_closure: VerifiedCoreStrongRequirementClosureV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
) -> Result<VerifiedSourceExternalRequirementClosureV1, SourceExternalRequirementValidationError> {
    if core_closure.producer() != native_requirements.producer() {
        return Err(SourceExternalRequirementValidationError::ProducerMismatch {
            object: core_closure.producer(),
            native: native_requirements.producer(),
        });
    }
    if core_closure.target() != native_requirements.target() {
        return Err(SourceExternalRequirementValidationError::TargetMismatch {
            object: core_closure.target(),
            native: native_requirements.target(),
        });
    }

    let requirements = native_requirements
        .contracts()
        .iter()
        .map(|requirement| {
            (
                requirement
                    .symbol_key()
                    .native_link_symbol()
                    .as_bytes()
                    .to_vec(),
                requirement,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut source_external_requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in core_closure.remaining_external_candidates() {
        if let Some(requirement) = requirements.get(binding.symbol()) {
            source_external_requirements.push(SourceExternalRequirementUseV1 {
                use_site: binding.clone(),
                requirement: (*requirement).clone(),
            });
        } else {
            remaining_external_candidates.push(binding.clone());
        }
    }

    Ok(VerifiedSourceExternalRequirementClosureV1 {
        core_closure,
        native_requirements,
        source_external_requirements,
        remaining_external_candidates,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceExternalRequirementValidationError {
    ProducerMismatch {
        object: ConeIdentity,
        native: ConeIdentity,
    },
    TargetMismatch {
        object: LirTargetProfile,
        native: LirTargetProfile,
    },
}

impl fmt::Display for SourceExternalRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid source external requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for SourceExternalRequirementValidationError {}

#[cfg(test)]
mod tests;
