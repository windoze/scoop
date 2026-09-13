//! Closed runtime-ABI and target-EH classification for external relocations.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    RuntimeRequirementRegistryError, RuntimeSymbolContractRegistryV1, RuntimeSymbolContractV1,
    TargetEhRequirementRegistryV1, TargetEhRequirementV1, ValidatedLirTargetSelection,
};

use super::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1,
    VerifiedSourceExternalRequirementClosureV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAbiRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    contract: RuntimeSymbolContractV1,
}

impl RuntimeAbiRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn contract(&self) -> &RuntimeSymbolContractV1 {
        &self.contract
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetEhRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: TargetEhRequirementV1,
}

impl TargetEhRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn requirement(&self) -> &TargetEhRequirementV1 {
        &self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeAndEhRequirementClosureV1 {
    source_closure: VerifiedSourceExternalRequirementClosureV1,
    selection: ValidatedLirTargetSelection,
    runtime_requirements: Vec<RuntimeAbiRequirementUseV1>,
    target_eh_requirements: Vec<TargetEhRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedBuiltinObjectExternalRequirementClosureV1 {
    verified: VerifiedRuntimeAndEhRequirementClosureV1,
}

impl SealedBuiltinObjectExternalRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.verified.producer()
    }

    pub const fn verified(&self) -> &VerifiedRuntimeAndEhRequirementClosureV1 {
        &self.verified
    }
}

pub fn seal_builtin_object_external_requirements_v1(
    verified: VerifiedRuntimeAndEhRequirementClosureV1,
) -> Result<
    SealedBuiltinObjectExternalRequirementClosureV1,
    BuiltinObjectExternalRequirementClosureError,
> {
    if let Some(binding) = verified.remaining_external_candidates().first() {
        return Err(
            BuiltinObjectExternalRequirementClosureError::UnclassifiedExternalRelocation {
                member: binding.source_member(),
                atom: binding.containing_atom(),
                symbol: binding.symbol().to_vec(),
            },
        );
    }
    Ok(SealedBuiltinObjectExternalRequirementClosureV1 { verified })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinObjectExternalRequirementClosureError {
    UnclassifiedExternalRelocation {
        member: crate::SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        symbol: Vec<u8>,
    },
}

impl fmt::Display for BuiltinObjectExternalRequirementClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "incomplete built-in object external requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for BuiltinObjectExternalRequirementClosureError {}

impl VerifiedRuntimeAndEhRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.source_closure.producer()
    }

    pub const fn selection(&self) -> ValidatedLirTargetSelection {
        self.selection
    }

    pub const fn source_closure(&self) -> &VerifiedSourceExternalRequirementClosureV1 {
        &self.source_closure
    }

    pub fn runtime_requirements(&self) -> &[RuntimeAbiRequirementUseV1] {
        &self.runtime_requirements
    }

    pub fn target_eh_requirements(&self) -> &[TargetEhRequirementUseV1] {
        &self.target_eh_requirements
    }

    pub fn remaining_external_candidates(&self) -> &[StrongRelocationBindingV1] {
        &self.remaining_external_candidates
    }
}

pub fn verify_runtime_and_eh_requirements_v1(
    source_closure: VerifiedSourceExternalRequirementClosureV1,
    selection: ValidatedLirTargetSelection,
) -> Result<VerifiedRuntimeAndEhRequirementClosureV1, RuntimeAndEhRequirementValidationError> {
    if source_closure.target() != selection.target() {
        return Err(RuntimeAndEhRequirementValidationError::TargetMismatch {
            object: source_closure.target(),
            selection: selection.target(),
        });
    }

    let runtime_registry = RuntimeSymbolContractRegistryV1::current(selection.target())?;
    let eh_registry = TargetEhRequirementRegistryV1::current(selection)?;
    let mut runtime_requirements = Vec::new();
    let mut target_eh_requirements = Vec::new();
    let mut remaining_external_candidates = Vec::new();
    for binding in source_closure.remaining_external_candidates() {
        let runtime = runtime_registry.contract_for_object_symbol(binding.symbol());
        let eh = eh_registry.requirement_for_object_symbol(binding.symbol());
        match (runtime, eh) {
            (Some(contract), None) => {
                runtime_requirements.push(RuntimeAbiRequirementUseV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    contract: contract.clone(),
                });
            }
            (None, Some(requirement)) => {
                target_eh_requirements.push(TargetEhRequirementUseV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    requirement: requirement.clone(),
                });
            }
            (None, None) => remaining_external_candidates.push(binding.clone()),
            (Some(_), Some(_)) => {
                return Err(
                    RuntimeAndEhRequirementValidationError::AmbiguousRegisteredObjectSymbol {
                        symbol: binding.symbol().to_vec(),
                    },
                );
            }
        }
    }

    Ok(VerifiedRuntimeAndEhRequirementClosureV1 {
        source_closure,
        selection,
        runtime_requirements,
        target_eh_requirements,
        remaining_external_candidates,
    })
}

#[derive(Debug)]
pub enum RuntimeAndEhRequirementValidationError {
    TargetMismatch {
        object: scoop_lir::LirTargetProfile,
        selection: scoop_lir::LirTargetProfile,
    },
    Registry(RuntimeRequirementRegistryError),
    AmbiguousRegisteredObjectSymbol {
        symbol: Vec<u8>,
    },
}

impl From<RuntimeRequirementRegistryError> for RuntimeAndEhRequirementValidationError {
    fn from(error: RuntimeRequirementRegistryError) -> Self {
        Self::Registry(error)
    }
}

impl fmt::Display for RuntimeAndEhRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime or target EH requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for RuntimeAndEhRequirementValidationError {}

#[cfg(test)]
pub(super) mod tests;
