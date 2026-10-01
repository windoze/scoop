use scoop_identity::StrongDefinitionEntityKind;
use scoop_wire::{RuntimeEncodeError, RuntimeEncoder};

use super::super::StrongDefinitionOwnerV1;

pub(super) fn encode_strong_owner(
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
