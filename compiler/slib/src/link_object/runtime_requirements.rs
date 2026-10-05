//! Closed runtime-ABI and target-EH classification for external relocations.

use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    RuntimeAbiSymbolV1, RuntimeRequirementRegistryError, RuntimeSymbolContractRegistryV1,
    RuntimeSymbolContractV1, TargetEhRequirementRegistryV1, TargetEhRequirementV1,
    ValidatedLirTargetSelection,
};

use super::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1, VerifiedObjectRelocationFormV1,
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
                validate_runtime_relocation(
                    contract,
                    binding.relocation_form(),
                    selection.target(),
                )?;
                runtime_requirements.push(RuntimeAbiRequirementUseV1 {
                    use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                    contract: contract.clone(),
                });
            }
            (None, Some(requirement)) => {
                if binding
                    .relocation_form()
                    .is_tls_reference(selection.target())
                {
                    return Err(
                        RuntimeAndEhRequirementValidationError::TlsRelocationRequiresTlsContract {
                            symbol: binding.symbol().to_vec(),
                            form: binding.relocation_form(),
                        },
                    );
                }
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

fn validate_runtime_relocation(
    contract: &RuntimeSymbolContractV1,
    form: VerifiedObjectRelocationFormV1,
    target: scoop_lir::LirTargetProfile,
) -> Result<(), RuntimeAndEhRequirementValidationError> {
    let is_allocation_context = contract.symbol() == RuntimeAbiSymbolV1::AllocationContext;
    if form.is_tls_reference(target) != is_allocation_context {
        return Err(
            RuntimeAndEhRequirementValidationError::RuntimeRelocationFormMismatch {
                symbol: contract.symbol(),
                form,
            },
        );
    }
    Ok(())
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
    RuntimeRelocationFormMismatch {
        symbol: RuntimeAbiSymbolV1,
        form: VerifiedObjectRelocationFormV1,
    },
    TlsRelocationRequiresTlsContract {
        symbol: Vec<u8>,
        form: VerifiedObjectRelocationFormV1,
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
