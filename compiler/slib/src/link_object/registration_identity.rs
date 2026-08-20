use scoop_lir::RegistrationDefinitionOwner;
use scoop_wire::{RuntimeEncodeError, RuntimeEncoder};

pub(super) fn runtime_encode_registration_identity(
    encoder: &mut RuntimeEncoder,
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> Result<(), RuntimeEncodeError> {
    let (linkage, group, member) = match owner {
        RegistrationDefinitionOwner::Strong => (1, [0; 32], [0; 32]),
        RegistrationDefinitionOwner::Odr { group, member } => {
            (2, *group.as_array(), *member.as_array())
        }
    };
    encoder.u32(linkage)?;
    encoder.fixed(semantic)?;
    encoder.fixed(&group)?;
    encoder.fixed(&member)
}

pub(super) fn provisional_registration_identity(
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> [u8; 104] {
    let mut bytes = [0; 104];
    let linkage = match owner {
        RegistrationDefinitionOwner::Strong => 1_u32,
        RegistrationDefinitionOwner::Odr { group, member } => {
            bytes[40..72].copy_from_slice(group.as_array());
            bytes[72..104].copy_from_slice(member.as_array());
            2
        }
    };
    bytes[..4].copy_from_slice(&linkage.to_le_bytes());
    bytes[8..40].copy_from_slice(semantic);
    bytes
}
