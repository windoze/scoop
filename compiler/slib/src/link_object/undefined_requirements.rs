//! Canonical final undefined-symbol requirements for one verified artifact.

use std::fmt;

use scoop_identity::{
    ConeIdentity, GeneratedBridgeUnitId, NativeExternalContractFingerprint, NativeLibraryBinding,
};
use scoop_lir::{
    CBridgeTargetSupportRequirementId, RuntimeSymbolContractId, TargetEhRequirementId,
    ValidatedLirTargetSelection,
};
use scoop_wire::{Encoder, WireEncode};

use super::{
    BuiltinObjectSectionRoleV1, CanonicalUndefinedRelocationUseV1,
    CurrentConeUndefinedRequirementV1, RelocationTargetSlotV1,
    SealedBuiltinObjectExternalRequirementClosureV1, StrongDefinitionOwnerV1,
    StrongRelocationResolutionV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
    VerifiedDarwinArm64RelocationFormV1,
};
use crate::SlibMemberId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalUndefinedSymbolRequirementV1 {
    IntraConeStrong {
        owner: StrongDefinitionOwnerV1,
    },
    CoreStrong {
        core: ConeIdentity,
        owner: StrongDefinitionOwnerV1,
    },
    GeneratedBridge {
        unit: GeneratedBridgeUnitId,
    },
    SourceExtern {
        contract: NativeExternalContractFingerprint,
        library: NativeLibraryBinding,
    },
    RuntimeAbi {
        contract: RuntimeSymbolContractId,
    },
    TargetEhSupport {
        contract: TargetEhRequirementId,
    },
    CBridgeTargetSupport {
        contract: CBridgeTargetSupportRequirementId,
    },
}

impl WireEncode for FinalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::IntraConeStrong { owner } => encode_one_field_sum(encoder, 1, owner),
            Self::CoreStrong { core, owner } => {
                encoder.map(3)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                core.encode(encoder)?;
                encoder.field(2)?;
                owner.encode(encoder)
            }
            Self::GeneratedBridge { unit } => encode_one_field_sum(encoder, 3, unit),
            Self::SourceExtern { contract, library } => {
                encoder.map(3)?;
                encode_tag(encoder, 4)?;
                encoder.field(1)?;
                contract.encode(encoder)?;
                encoder.field(2)?;
                library.encode(encoder)
            }
            Self::RuntimeAbi { contract } => encode_one_field_sum(encoder, 5, contract),
            Self::TargetEhSupport { contract } => encode_one_field_sum(encoder, 6, contract),
            Self::CBridgeTargetSupport { contract } => encode_one_field_sum(encoder, 7, contract),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalUndefinedSymbolRequirementV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: FinalUndefinedSymbolRequirementV1,
}

impl CanonicalUndefinedSymbolRequirementV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.use_site.source_member()
    }

    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn requirement(&self) -> FinalUndefinedSymbolRequirementV1 {
        self.requirement
    }
}

impl WireEncode for CanonicalUndefinedSymbolRequirementV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.use_site.encode(encoder)?;
        encoder.field(2)?;
        self.requirement.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalUndefinedSymbolRequirementSetV1 {
    producer: ConeIdentity,
    selection: ValidatedLirTargetSelection,
    requirements: Vec<CanonicalUndefinedSymbolRequirementV1>,
}

impl CanonicalUndefinedSymbolRequirementSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn selection(&self) -> ValidatedLirTargetSelection {
        self.selection
    }

    pub fn requirements(&self) -> &[CanonicalUndefinedSymbolRequirementV1] {
        &self.requirements
    }
}

impl WireEncode for CanonicalUndefinedSymbolRequirementSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.requirements.len() as u64)?;
        for requirement in &self.requirements {
            requirement.encode(encoder)?;
        }
        Ok(())
    }
}

pub fn finalize_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
) -> Result<CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementFinalizationError> {
    let external_strong = external.verified().strong_closure();
    if current_cone.strong_closure() != external_strong {
        return Err(UndefinedSymbolRequirementFinalizationError::StrongClosureMismatch);
    }

    let mut requirements = Vec::new();
    for item in current_cone.requirements() {
        let requirement = match item.requirement() {
            CurrentConeUndefinedRequirementV1::IntraConeStrong { owner } => {
                FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner }
            }
            CurrentConeUndefinedRequirementV1::GeneratedBridge { unit } => {
                FinalUndefinedSymbolRequirementV1::GeneratedBridge { unit }
            }
        };
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement,
        });
    }

    let verified_external = external.verified();
    for item in verified_external.core_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::CoreStrong {
                core: ConeIdentity::CORE,
                owner: item.owner(),
            },
        });
    }
    for item in verified_external.source_external_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::SourceExtern {
                contract: item.requirement().fingerprint(),
                library: item.requirement().library().binding(),
            },
        });
    }
    for item in verified_external.runtime_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::RuntimeAbi {
                contract: item.contract().id(),
            },
        });
    }
    for item in verified_external.target_eh_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::TargetEhSupport {
                contract: item.requirement().id(),
            },
        });
    }
    for item in verified_external.target_support_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport {
                contract: item.requirement().id(),
            },
        });
    }

    requirements.sort_unstable_by_key(|item| use_key(item.use_site()));
    if let Some(pair) = requirements
        .windows(2)
        .find(|pair| use_key(pair[0].use_site()) == use_key(pair[1].use_site()))
    {
        let use_site = pair[0].use_site();
        return Err(UndefinedSymbolRequirementFinalizationError::DuplicateUse {
            member: use_site.source_member(),
            atom: use_site.containing_atom(),
            offset: use_site.offset_within_atom(),
            target_slot: use_site.target_slot(),
        });
    }

    let mut expected = current_cone
        .strong_closure()
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
    let actual = requirements
        .iter()
        .map(|item| item.use_site().clone())
        .collect::<Vec<_>>();
    if actual != expected {
        let first_mismatch = actual
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
            .or_else(|| {
                (actual.len() != expected.len()).then_some(actual.len().min(expected.len()))
            });
        return Err(
            UndefinedSymbolRequirementFinalizationError::RequirementCoverageMismatch {
                expected: expected.len(),
                actual: actual.len(),
                first_mismatch,
            },
        );
    }

    Ok(CanonicalUndefinedSymbolRequirementSetV1 {
        producer: current_cone.producer(),
        selection: verified_external.selection(),
        requirements,
    })
}

type UseKey = (
    SlibMemberId,
    scoop_identity::ObjectDefinitionAtomId,
    u64,
    RelocationTargetSlotV1,
);

fn use_key(use_site: &CanonicalUndefinedRelocationUseV1) -> UseKey {
    (
        use_site.source_member(),
        use_site.containing_atom(),
        use_site.offset_within_atom(),
        use_site.target_slot(),
    )
}

impl WireEncode for CanonicalUndefinedRelocationUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encoder.field(1)?;
        self.source_member().encode(encoder)?;
        encoder.field(2)?;
        self.containing_atom().encode(encoder)?;
        encoder.field(3)?;
        self.containing_atom_role().encode(encoder)?;
        encoder.field(4)?;
        encode_section_role(encoder, self.section_role())?;
        encoder.field(5)?;
        encoder.unsigned(self.offset_within_atom())?;
        encoder.field(6)?;
        encoder.unsigned(u64::from(self.width_bytes()))?;
        encoder.field(7)?;
        encode_relocation_form(encoder, self.relocation_form())?;
        encoder.field(8)?;
        encoder.unsigned(self.encoded_value())?;
        encoder.field(9)?;
        encoder.unsigned(match self.target_slot() {
            RelocationTargetSlotV1::Single => 1,
            RelocationTargetSlotV1::Minuend => 2,
            RelocationTargetSlotV1::Subtrahend => 3,
        })?;
        encoder.field(10)?;
        encoder.bytes(self.symbol())
    }
}

fn encode_section_role(
    encoder: &mut Encoder,
    role: BuiltinObjectSectionRoleV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.unsigned(match role {
        BuiltinObjectSectionRoleV1::Text => 1,
        BuiltinObjectSectionRoleV1::ReadOnlyData => 2,
        BuiltinObjectSectionRoleV1::CString => 3,
        BuiltinObjectSectionRoleV1::WritableData => 4,
        BuiltinObjectSectionRoleV1::ZeroFill => 5,
        BuiltinObjectSectionRoleV1::GccExceptionTable => 6,
        BuiltinObjectSectionRoleV1::LlvmStackmaps => 7,
        BuiltinObjectSectionRoleV1::CompactUnwind => 8,
        BuiltinObjectSectionRoleV1::EhFrame => 9,
    })
}

fn encode_relocation_form(
    encoder: &mut Encoder,
    form: VerifiedDarwinArm64RelocationFormV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    match form {
        VerifiedDarwinArm64RelocationFormV1::Unsigned64 => encode_empty_sum(encoder, 1),
        VerifiedDarwinArm64RelocationFormV1::Subtractor64 => encode_empty_sum(encoder, 2),
        VerifiedDarwinArm64RelocationFormV1::Branch26 => encode_empty_sum(encoder, 3),
        VerifiedDarwinArm64RelocationFormV1::Page21 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 4, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::PageOffset12 { explicit_addend } => {
            encode_optional_addend_sum(encoder, 5, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::GotLoadPage21 => encode_empty_sum(encoder, 6),
        VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12 => encode_empty_sum(encoder, 7),
        VerifiedDarwinArm64RelocationFormV1::PointerToGot32 => encode_empty_sum(encoder, 8),
    }
}

fn encode_optional_addend_sum(
    encoder: &mut Encoder,
    tag: u64,
    addend: Option<i32>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    match addend {
        Some(addend) if addend >= 0 => {
            encoder.map(2)?;
            encode_tag(encoder, 2)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(addend.unsigned_abs()))
        }
        Some(addend) => {
            encoder.map(2)?;
            encode_tag(encoder, 3)?;
            encoder.field(1)?;
            encoder.unsigned(u64::from(addend.unsigned_abs()))
        }
        None => encode_empty_sum(encoder, 1),
    }
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_one_field_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndefinedSymbolRequirementFinalizationError {
    StrongClosureMismatch,
    DuplicateUse {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    RequirementCoverageMismatch {
        expected: usize,
        actual: usize,
        first_mismatch: Option<usize>,
    },
}

impl fmt::Display for UndefinedSymbolRequirementFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid final undefined requirement set: {self:?}"
        )
    }
}

impl std::error::Error for UndefinedSymbolRequirementFinalizationError {}

#[cfg(test)]
mod tests;
