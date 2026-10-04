//! Typed resolution of source-native external relocation requirements.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{
    CanonicalNativeExternalRequirementSurfaceV1, CanonicalNativeExternalRequirementV1,
    LirTargetProfile,
};

use super::{
    CanonicalUndefinedRelocationUseV1, StrongRelocationBindingV1,
    VerifiedCrossConeStrongRequirementClosureV1, VerifiedExternalShapeRequirementClosureV1,
    VerifiedObjectRelocationFormV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceExternalRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: CanonicalNativeExternalRequirementV1,
}

impl SourceExternalRequirementUseV1 {
    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn requirement(&self) -> &CanonicalNativeExternalRequirementV1 {
        &self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSourceExternalRequirementClosureV1 {
    cross_cone_closure: VerifiedCrossConeStrongRequirementClosureV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    source_external_requirements: Vec<SourceExternalRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedSourceExternalRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.cross_cone_closure.producer()
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.cross_cone_closure.target()
    }

    pub const fn cross_cone_closure(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        &self.cross_cone_closure
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
    cross_cone_closure: VerifiedCrossConeStrongRequirementClosureV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
) -> Result<VerifiedSourceExternalRequirementClosureV1, SourceExternalRequirementValidationError> {
    let candidates = cross_cone_closure.remaining_external_candidates().to_vec();
    verify_source_external_requirements_from_candidates_v1(
        cross_cone_closure,
        candidates,
        native_requirements,
    )
}

/// Continues legacy native/runtime classification only after the general
/// shape partition has claimed its complete physical-use set.
pub fn verify_source_external_requirements_after_external_shape_v1(
    external_shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
) -> Result<VerifiedSourceExternalRequirementClosureV1, SourceExternalRequirementValidationError> {
    let candidates = external_shape
        .remaining_external_candidates()
        .iter()
        .map(|binding| (*binding).clone())
        .collect::<Vec<_>>();
    verify_source_external_requirements_from_candidates_v1(
        external_shape.legacy_closure().clone(),
        candidates,
        native_requirements,
    )
}

fn verify_source_external_requirements_from_candidates_v1(
    cross_cone_closure: VerifiedCrossConeStrongRequirementClosureV1,
    candidates: Vec<StrongRelocationBindingV1>,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
) -> Result<VerifiedSourceExternalRequirementClosureV1, SourceExternalRequirementValidationError> {
    if cross_cone_closure.producer() != native_requirements.producer() {
        return Err(SourceExternalRequirementValidationError::ProducerMismatch {
            object: cross_cone_closure.producer(),
            native: native_requirements.producer(),
        });
    }
    if cross_cone_closure.target() != native_requirements.target() {
        return Err(SourceExternalRequirementValidationError::TargetMismatch {
            object: cross_cone_closure.target(),
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
    for binding in candidates {
        if let Some(requirement) = requirements.get(binding.symbol()) {
            if is_tlvp_relocation(binding.relocation_form())
                && !matches!(
                    requirement.contract(),
                    scoop_identity::NativeExternalContract::ReadOnlyTls { .. }
                        | scoop_identity::NativeExternalContract::MutableTls { .. }
                )
            {
                return Err(
                    SourceExternalRequirementValidationError::TlvpRelocationRequiresTlsContract {
                        contract: requirement.fingerprint(),
                        form: binding.relocation_form(),
                    },
                );
            }
            source_external_requirements.push(SourceExternalRequirementUseV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(&binding),
                requirement: (*requirement).clone(),
            });
        } else {
            remaining_external_candidates.push(binding);
        }
    }

    Ok(VerifiedSourceExternalRequirementClosureV1 {
        cross_cone_closure,
        native_requirements,
        source_external_requirements,
        remaining_external_candidates,
    })
}

const fn is_tlvp_relocation(form: VerifiedObjectRelocationFormV1) -> bool {
    matches!(
        form,
        VerifiedObjectRelocationFormV1::TlvpLoadPage21
            | VerifiedObjectRelocationFormV1::TlvpLoadPageOffset12
    )
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
    TlvpRelocationRequiresTlsContract {
        contract: scoop_identity::NativeExternalContractFingerprint,
        form: VerifiedObjectRelocationFormV1,
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
pub(super) mod tests;
