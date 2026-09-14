use scoop_identity::{
    DigestKind, DigestNodeId, NativeLibraryBinding, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentStaticStorageId, StrongDefinitionEntity,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use scoop_wire::{RuntimeEncode, RuntimeEncodeError, RuntimeEncoder};

use crate::link_object::{
    CanonicalUndefinedRelocationUseV1, CanonicalUndefinedSymbolRequirementSetV1,
    FinalUndefinedSymbolRequirementV1, LinkDefinitionOwnerV1, RelocationTargetSlotV1,
    StrongDefinitionOwnerV1, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedDarwinArm64RelocationFormV1,
    VerifiedDarwinArm64RelocationShapeV1, VerifiedMemberObjectRelocationIndexV1,
    VerifiedRelocationTargetV1,
};

const PRIMARY_ATOM_ROLE: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectDefinitionRelocationFailureV1 {
    BindingCount,
    UnsupportedLocalOrSectionTarget,
    UnsupportedObjectLocalOwner,
    MissingUndefinedRequirement,
    Range,
    EncodedValue,
}

pub(in crate::link_object) struct ObjectDefinitionFingerprintInputV1<'a> {
    pub(in crate::link_object) bytes: &'a [u8],
    pub(in crate::link_object) relocations: &'a [CanonicalObjectRelocationV1],
    pub(in crate::link_object) direct_inputs: &'a [CanonicalDigestInputV1],
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

#[derive(Clone, Copy)]
pub(in crate::link_object) struct CanonicalDigestInputV1 {
    pub(in crate::link_object) kind: DigestKind,
    pub(in crate::link_object) node: DigestNodeId,
    pub(in crate::link_object) digest: [u8; 32],
}

impl RuntimeEncode for CanonicalDigestInputV1 {
    fn runtime_encode(&self, encoder: &mut RuntimeEncoder) -> Result<(), RuntimeEncodeError> {
        encoder.u32(self.kind.tag())?;
        encoder.fixed(self.node.as_array())?;
        encoder.fixed(&self.digest)
    }
}

#[derive(Clone)]
pub(in crate::link_object) struct CanonicalObjectRelocationV1 {
    offset_within_atom: u64,
    form: VerifiedDarwinArm64RelocationFormV1,
    encoded_value: u64,
    targets: Vec<CanonicalRelocationTargetV1>,
}

#[derive(Clone, Copy)]
pub(in crate::link_object) enum CanonicalStaticStorageTargetV1 {
    InitialTemplate(PersistentStaticStorageId),
    InitialRelocationTable(PersistentStaticStorageId),
    EmptyTemplateSentinel,
    EmptyRelocationTableSentinel,
}

impl CanonicalObjectRelocationV1 {
    pub(super) fn callable_entry(body: PersistentCallableBodyId) -> Self {
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::callable_body(body),
            StrongDefinitionRole::CallableBody,
        )
        .expect("callable bodies are valid strong definition owners");
        Self::unsigned64(
            184,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
        )
    }

    pub(in crate::link_object) fn type_descriptor(exact_type: PersistentExactTypeId) -> Self {
        let owner = StrongDefinitionOwnerV1::new(
            StrongDefinitionEntity::exact_type(exact_type),
            StrongDefinitionRole::TypeDescriptor,
        )
        .expect("type descriptors are valid strong definition owners");
        Self::unsigned64(
            168,
            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner },
        )
    }

    pub(in crate::link_object) fn unsigned64(
        offset_within_atom: u64,
        requirement: FinalUndefinedSymbolRequirementV1,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::Requirement(requirement),
            }],
        }
    }

    pub(in crate::link_object) fn static_storage_target(
        offset_within_atom: u64,
        target: CanonicalStaticStorageTargetV1,
    ) -> Self {
        Self {
            offset_within_atom,
            form: VerifiedDarwinArm64RelocationFormV1::Unsigned64,
            encoded_value: 0,
            targets: vec![CanonicalRelocationTargetV1 {
                slot: RelocationTargetSlotV1::Single,
                target: CanonicalRelocationTargetKindV1::StaticStorage(target),
            }],
        }
    }

    pub(in crate::link_object) fn normalize_bytes(
        &self,
        bytes: &mut [u8],
    ) -> Result<(), ObjectDefinitionRelocationFailureV1> {
        let start = usize::try_from(self.offset_within_atom)
            .map_err(|_| ObjectDefinitionRelocationFailureV1::Range)?;
        match self.form {
            VerifiedDarwinArm64RelocationFormV1::Unsigned64
            | VerifiedDarwinArm64RelocationFormV1::Subtractor64 => {
                let slot = bytes
                    .get_mut(start..start + 8)
                    .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
                let actual = u64::from_le_bytes(slot.try_into().expect("eight-byte slot"));
                if actual != self.encoded_value {
                    return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
                }
                slot.fill(0);
            }
            VerifiedDarwinArm64RelocationFormV1::Branch26 => {
                normalize_u32(bytes, start, self.encoded_value, 0xfc00_0000)?;
            }
            VerifiedDarwinArm64RelocationFormV1::Page21 { .. }
            | VerifiedDarwinArm64RelocationFormV1::GotLoadPage21 => {
                normalize_u32(bytes, start, self.encoded_value, 0x9f00_001f)?;
            }
            VerifiedDarwinArm64RelocationFormV1::PageOffset12 { .. }
            | VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12 => {
                normalize_u32(bytes, start, self.encoded_value, 0xffc0_03ff)?;
            }
            VerifiedDarwinArm64RelocationFormV1::PointerToGot32 => {
                normalize_u32(bytes, start, self.encoded_value, 0)?;
            }
        }
        Ok(())
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

#[derive(Clone)]
struct CanonicalRelocationTargetV1 {
    slot: RelocationTargetSlotV1,
    target: CanonicalRelocationTargetKindV1,
}

#[derive(Clone, Copy)]
enum CanonicalRelocationTargetKindV1 {
    Requirement(FinalUndefinedSymbolRequirementV1),
    StaticStorage(CanonicalStaticStorageTargetV1),
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
                encode_requirement(encoder, requirement)
            }
            CanonicalRelocationTargetKindV1::StaticStorage(target) => {
                encode_static_storage_target(encoder, target)
            }
        }
    }
}

pub(in crate::link_object) fn canonicalize_relocations(
    bytes: &[u8],
    member: &VerifiedMemberObjectRelocationIndexV1,
    atom: scoop_identity::ObjectDefinitionAtomId,
    closure: &VerifiedCurrentConeStrongRelocationClosureV1,
    requirements: &CanonicalUndefinedSymbolRequirementSetV1,
) -> Result<Vec<CanonicalObjectRelocationV1>, ObjectDefinitionRelocationFailureV1> {
    let mut output = Vec::new();
    for relocation in member
        .relocations()
        .iter()
        .filter(|relocation| relocation.containing_atom() == atom)
    {
        if shape_has_local_or_section_target(relocation.shape()) {
            return Err(ObjectDefinitionRelocationFailureV1::UnsupportedLocalOrSectionTarget);
        }
        let bindings = closure
            .bindings()
            .iter()
            .filter(|binding| {
                binding.source_member() == member.member()
                    && binding.containing_atom() == atom
                    && binding.offset_within_atom() == relocation.offset_within_atom()
            })
            .collect::<Vec<_>>();
        let expected_count = usize::from(matches!(
            relocation.shape().form(),
            VerifiedDarwinArm64RelocationFormV1::Subtractor64
        )) + 1;
        if bindings.len() != expected_count {
            return Err(ObjectDefinitionRelocationFailureV1::BindingCount);
        }
        let mut targets = bindings
            .into_iter()
            .map(|binding| {
                let requirement = match binding.resolution() {
                    StrongRelocationResolutionV1::ObjectLocalStrong { owner, .. } => match owner {
                        LinkDefinitionOwnerV1::StrongDefinition(owner) => {
                            FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner }
                        }
                        _ => {
                            return Err(
                                ObjectDefinitionRelocationFailureV1::UnsupportedObjectLocalOwner,
                            );
                        }
                    },
                    StrongRelocationResolutionV1::CurrentConeUndefinedStrong { .. }
                    | StrongRelocationResolutionV1::ExternalCandidate { .. } => {
                        let use_site = CanonicalUndefinedRelocationUseV1::from(binding);
                        requirements
                            .requirements()
                            .iter()
                            .find(|requirement| requirement.use_site() == &use_site)
                            .map(|requirement| requirement.requirement())
                            .ok_or(
                                ObjectDefinitionRelocationFailureV1::MissingUndefinedRequirement,
                            )?
                    }
                };
                Ok(CanonicalRelocationTargetV1 {
                    slot: binding.target_slot(),
                    target: CanonicalRelocationTargetKindV1::Requirement(requirement),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        targets.sort_unstable_by_key(|target| target.slot);
        output.push(CanonicalObjectRelocationV1 {
            offset_within_atom: relocation.offset_within_atom(),
            form: relocation.shape().form(),
            encoded_value: relocation.encoded_value(),
            targets,
        });
    }
    output.sort_unstable_by_key(|relocation| relocation.offset_within_atom);
    for relocation in &output {
        let start = usize::try_from(relocation.offset_within_atom)
            .map_err(|_| ObjectDefinitionRelocationFailureV1::Range)?;
        let width = match relocation.form {
            VerifiedDarwinArm64RelocationFormV1::Unsigned64
            | VerifiedDarwinArm64RelocationFormV1::Subtractor64 => 8,
            _ => 4,
        };
        if bytes.get(start..start + width).is_none() {
            return Err(ObjectDefinitionRelocationFailureV1::Range);
        }
    }
    Ok(output)
}

fn shape_has_local_or_section_target(shape: &VerifiedDarwinArm64RelocationShapeV1) -> bool {
    match shape {
        VerifiedDarwinArm64RelocationShapeV1::Unsigned64 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Branch26 { target }
        | VerifiedDarwinArm64RelocationShapeV1::Page21 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::PageOffset12 { target, .. }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPage21 { target }
        | VerifiedDarwinArm64RelocationShapeV1::GotLoadPageOffset12 { target }
        | VerifiedDarwinArm64RelocationShapeV1::PointerToGot32 { target } => {
            is_local_or_section(target)
        }
        VerifiedDarwinArm64RelocationShapeV1::Subtractor64 {
            minuend,
            subtrahend,
        } => is_local_or_section(minuend) || is_local_or_section(subtrahend),
    }
}

fn is_local_or_section(target: &VerifiedRelocationTargetV1) -> bool {
    matches!(
        target,
        VerifiedRelocationTargetV1::LocalDefinition { .. }
            | VerifiedRelocationTargetV1::SectionBase { .. }
    )
}

fn normalize_u32(
    bytes: &mut [u8],
    start: usize,
    expected: u64,
    keep_mask: u32,
) -> Result<(), ObjectDefinitionRelocationFailureV1> {
    let slot = bytes
        .get_mut(start..start + 4)
        .ok_or(ObjectDefinitionRelocationFailureV1::Range)?;
    let actual = u32::from_le_bytes(slot.try_into().expect("four-byte slot"));
    if u64::from(actual) != expected {
        return Err(ObjectDefinitionRelocationFailureV1::EncodedValue);
    }
    slot.copy_from_slice(&(actual & keep_mask).to_le_bytes());
    Ok(())
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

fn encode_requirement(
    encoder: &mut RuntimeEncoder,
    requirement: FinalUndefinedSymbolRequirementV1,
) -> Result<(), RuntimeEncodeError> {
    match requirement {
        FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner } => {
            encoder.u32(1)?;
            encode_strong_owner(encoder, owner)
        }
        FinalUndefinedSymbolRequirementV1::CoreStrong { core, owner } => {
            encoder.u32(2)?;
            encoder.fixed(core.as_array())?;
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

fn encode_strong_owner(
    encoder: &mut RuntimeEncoder,
    owner: StrongDefinitionOwnerV1,
) -> Result<(), RuntimeEncodeError> {
    match owner.entity().kind() {
        StrongDefinitionEntityKind::CallableBody(id) => encode_entity(encoder, 1, id.as_array())?,
        StrongDefinitionEntityKind::StaticStorage(id) => encode_entity(encoder, 2, id.as_array())?,
        StrongDefinitionEntityKind::ImmortalObject(id) => encode_entity(encoder, 3, id.as_array())?,
        StrongDefinitionEntityKind::ExactType(id) => encode_entity(encoder, 4, id.as_array())?,
        StrongDefinitionEntityKind::Layout(id) => encode_entity(encoder, 5, id.as_array())?,
        StrongDefinitionEntityKind::Scan(id) => encode_entity(encoder, 6, id.as_array())?,
        StrongDefinitionEntityKind::DispatchTable(id) => encode_entity(encoder, 7, id.as_array())?,
        StrongDefinitionEntityKind::DispatchSlot(id) => encode_entity(encoder, 8, id.as_array())?,
        StrongDefinitionEntityKind::InitializationUnit(id) => {
            encode_entity(encoder, 9, id.as_array())?
        }
        StrongDefinitionEntityKind::SafepointSite(id) => encode_entity(encoder, 10, id.as_array())?,
        StrongDefinitionEntityKind::ConeImage(id) => encode_entity(encoder, 11, id.as_array())?,
        StrongDefinitionEntityKind::GeneratedBridgeAtom(id) => {
            encode_entity(encoder, 12, id.as_array())?
        }
        StrongDefinitionEntityKind::RootEntry(id) => encode_entity(encoder, 13, id.as_array())?,
    }
    encoder.u32(match owner.role() {
        scoop_identity::StrongDefinitionRole::CallableBody => 1,
        scoop_identity::StrongDefinitionRole::StaticStorage => 2,
        scoop_identity::StrongDefinitionRole::ImmortalObject => 3,
        scoop_identity::StrongDefinitionRole::TypeDescriptor => 4,
        scoop_identity::StrongDefinitionRole::Layout => 5,
        scoop_identity::StrongDefinitionRole::ScanProgram => 6,
        scoop_identity::StrongDefinitionRole::DispatchTable => 7,
        scoop_identity::StrongDefinitionRole::DispatchSlot => 8,
        scoop_identity::StrongDefinitionRole::InitializationCell => 9,
        scoop_identity::StrongDefinitionRole::InitializationDescriptor => 10,
        scoop_identity::StrongDefinitionRole::RootRegistration => 11,
        scoop_identity::StrongDefinitionRole::ImmortalRegistration => 12,
        scoop_identity::StrongDefinitionRole::InitializationRegistration => 13,
        scoop_identity::StrongDefinitionRole::TypeRegistration => 14,
        scoop_identity::StrongDefinitionRole::SafepointRegistration => 15,
        scoop_identity::StrongDefinitionRole::CallableRegistration => 16,
        scoop_identity::StrongDefinitionRole::ImageDescriptor => 17,
        scoop_identity::StrongDefinitionRole::GeneratedBridge => 18,
        scoop_identity::StrongDefinitionRole::RootEntryDescriptor => 19,
    })
}

fn encode_entity(
    encoder: &mut RuntimeEncoder,
    tag: u32,
    id: &[u8; 32],
) -> Result<(), RuntimeEncodeError> {
    encoder.u32(tag)?;
    encoder.fixed(id)
}
