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
