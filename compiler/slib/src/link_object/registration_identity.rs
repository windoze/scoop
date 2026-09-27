use scoop_lir::RegistrationDefinitionOwner;

pub(super) fn provisional_registration_identity(
    semantic: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> [u8; 136] {
    let mut bytes = [0; 136];
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
