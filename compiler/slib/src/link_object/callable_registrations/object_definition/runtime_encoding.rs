use scoop_identity::{DefinitionAtomRole, NativeLibraryBinding};
use scoop_wire::{RuntimeEncode, RuntimeEncodeError, RuntimeEncoder};

use super::*;

mod strong_owner;
use strong_owner::encode_strong_owner;

impl RuntimeEncode for ObjectDefinitionLeafWithAssociatedAtomsInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        self.primary.runtime_encode(encoder)?;
        encoder.sequence_length(self.associated_atoms.len())?;
        for atom in self.associated_atoms {
            encoder.fixed(atom.atom.as_array())?;
            encoder.u32(definition_atom_role_tag(atom.role))?;
            encoder.byte_span(atom.bytes)?;
            encoder.sequence_length(atom.relocations.len())?;
            for relocation in atom.relocations {
                relocation.runtime_encode(encoder)?;
            }
        }
        Ok(())
    }
}

impl RuntimeEncode for ObjectDefinitionFingerprintInputV1<'_> {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(PRIMARY_ATOM_ROLE)?;
        encoder.byte_span(self.bytes)?;
        encoder.sequence_length(self.relocations.len())?;
        for relocation in self.relocations {
            relocation.runtime_encode(encoder)?;
        }
        encoder.sequence_length(self.direct_inputs.len())?;
        for input in self.direct_inputs {
            input.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

impl RuntimeEncode for CanonicalDigestInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.kind.tag())?;
        encoder.fixed(self.node.as_array())?;
        encoder.fixed(&self.digest)
    }
}

impl RuntimeEncode for CanonicalObjectRelocationV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u64(self.offset_within_atom)?;
        encode_relocation_form(encoder, self.form)?;
        encoder.u64(self.encoded_value)?;
        encoder.sequence_length(self.targets.len())?;
        for target in &self.targets {
            target.runtime_encode(encoder)?;
        }
        Ok(())
    }
}

impl RuntimeEncode for CanonicalRelocationTargetV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(match self.slot {
            RelocationTargetSlotV1::Single => 1,
            RelocationTargetSlotV1::Minuend => 2,
            RelocationTargetSlotV1::Subtrahend => 3,
        })?;
        match self.target {
            CanonicalRelocationTargetKindV1::Requirement(requirement) => {
                requirement.runtime_encode(encoder)
            }
            CanonicalRelocationTargetKindV1::StaticStorage(target) => {
                encode_static_storage_target(encoder, target)
            }
            CanonicalRelocationTargetKindV1::OwningAssociatedAtomOffset {
                atom,
                role,
                offset_within_atom,
            } => {
                encoder.u32(10)?;
                encoder.fixed(atom.as_array())?;
                encoder.u32(definition_atom_role_tag(role))?;
                encoder.u64(offset_within_atom)
            }
        }
    }
}

impl RuntimeEncode for CanonicalObjectDefinitionRequirementV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        let requirement = match *self {
            CanonicalObjectDefinitionRequirementV1::Legacy(requirement) => requirement,
            CanonicalObjectDefinitionRequirementV1::DependencyStrong { provider, target } => {
                encoder.u32(11)?;
                encoder.fixed(provider.as_array())?;
                return target.runtime_encode(encoder);
            }
            CanonicalObjectDefinitionRequirementV1::DependencyShapeStrong { provider, subject } => {
                encoder.u32(12)?;
                encoder.fixed(provider.as_array())?;
                return subject.runtime_encode(encoder);
            }
        };
        encode_legacy_requirement(encoder, requirement)
    }
}

fn encode_legacy_requirement(
    encoder: &mut RuntimeEncoder,
    requirement: FinalUndefinedSymbolRequirementV1,
) -> Result<(), RuntimeEncodeError> {
    match requirement {
        FinalUndefinedSymbolRequirementV1::OdrMember { member } => {
            encoder.u32(14)?;
            encoder.fixed(member.as_array())
        }
        FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner } => {
            encoder.u32(1)?;
            encode_strong_owner(encoder, owner)
        }

        FinalUndefinedSymbolRequirementV1::GeneratedBridge { unit } => {
            encoder.u32(3)?;
            encoder.fixed(unit.as_array())
        }
        FinalUndefinedSymbolRequirementV1::SourceExtern { contract, library } => {
            encoder.u32(4)?;
            encoder.fixed(contract.as_array())?;
            match library {
                NativeLibraryBinding::DefaultNativeNamespace => encoder.u32(1),
                NativeLibraryBinding::Requirement(requirement) => {
                    encoder.u32(2)?;
                    encoder.fixed(requirement.as_array())
                }
            }
        }
        FinalUndefinedSymbolRequirementV1::RuntimeAbi { contract } => {
            encoder.u32(5)?;
            encoder.fixed(contract.as_array())
        }
        FinalUndefinedSymbolRequirementV1::TargetEhSupport { contract } => {
            encoder.u32(6)?;
            encoder.fixed(contract.as_array())
        }
        FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport { contract } => {
            encoder.u32(7)?;
            encoder.fixed(contract.as_array())
        }
    }
}

fn encode_relocation_form(
    encoder: &mut RuntimeEncoder,
    form: VerifiedDarwinArm64RelocationFormV1,
) -> Result<(), RuntimeEncodeError> {
    match form {
        VerifiedDarwinArm64RelocationFormV1::Unsigned64 => encoder.u32(1),
        VerifiedDarwinArm64RelocationFormV1::Subtractor64 => encoder.u32(2),
        VerifiedDarwinArm64RelocationFormV1::Branch26 => encoder.u32(3),
        VerifiedDarwinArm64RelocationFormV1::Page21 { explicit_addend } => {
            encoder.u32(4)?;
            encode_optional_addend(encoder, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::PageOffset12 { explicit_addend } => {
            encoder.u32(5)?;
            encode_optional_addend(encoder, explicit_addend)
        }
        VerifiedDarwinArm64RelocationFormV1::GotLoadPage21 => encoder.u32(6),
        VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12 => encoder.u32(7),
        VerifiedDarwinArm64RelocationFormV1::PointerToGot32 => encoder.u32(8),
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21 => encoder.u32(9),
        VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12 => encoder.u32(10),
    }
}

fn encode_optional_addend(
    encoder: &mut RuntimeEncoder,
    addend: Option<i32>,
) -> Result<(), RuntimeEncodeError> {
    match addend {
        None => encoder.u32(1),
        Some(value) => {
            encoder.u32(2)?;
            encoder.u32(value as u32)
        }
    }
}

fn encode_static_storage_target(
    encoder: &mut RuntimeEncoder,
    target: CanonicalStaticStorageTargetV1,
) -> Result<(), RuntimeEncodeError> {
    match target {
        CanonicalStaticStorageTargetV1::InitialTemplate(storage) => {
            encoder.u32(8)?;
            encoder.u32(1)?;
            encoder.fixed(storage.as_array())
        }
        CanonicalStaticStorageTargetV1::InitialRelocationTable(storage) => {
            encoder.u32(8)?;
            encoder.u32(2)?;
            encoder.fixed(storage.as_array())
        }
        CanonicalStaticStorageTargetV1::EmptyTemplateSentinel => {
            encoder.u32(9)?;
            encoder.u32(1)
        }
        CanonicalStaticStorageTargetV1::EmptyRelocationTableSentinel => {
            encoder.u32(9)?;
            encoder.u32(2)
        }
    }
}

fn definition_atom_role_tag(role: DefinitionAtomRole) -> u32 {
    match role {
        DefinitionAtomRole::Primary => 1,
        DefinitionAtomRole::Lsda => 2,
        DefinitionAtomRole::EhFrame => 3,
        DefinitionAtomRole::CompactUnwind => 4,
        DefinitionAtomRole::Stackmap => 5,
        DefinitionAtomRole::RuntimeRecord => 6,
        DefinitionAtomRole::AddressTakenConstant => 7,
    }
}
